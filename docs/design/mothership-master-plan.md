# The mothership master plan: the whole ship, where everyone lives, and the fresh start

Status: a PLAN, written 2026-11-06. Nothing in it is built, and the wipe waits for the operator to
read it. Every operator decision is quoted with its date and source; everything else is marked
**Proposal** and is this document's reasoning, never his. The audit it rests on is
[world-audit-2026-11-06.md](world-audit-2026-11-06.md). **His answers to section 7, given the same
night, are in section 1.1**, and the plan below is changed to match them: the space program that
measures the ship (3.12), the crew-health rules (3.13), what the drive's thrust feels like aboard
(3.14), the apartment block (3.6) and drone mining ([drone-mining.md](drone-mining.md)). **His
second answers, 2026-11-07, are in section 1.2**: a sector is a community of 500 people (3.15),
the ship aims for 1 billion and 10 billion (3.16, a fleet), the cruise is a jump with no thrust
(3.14, a game fiction), and what was called a craft "cradle" is now a parking spot.

**Words since 2026-11-07.** A **sector** is a community of 500 residents (his unit). A **drum** is a
spinning cylinder that holds sectors. **Drum 1** is the 250 m x 2 km drum he sized on 2026-09-15,
which this plan called "Sector 1" until 2026-11-07. **Sector 1** is now the first 500-person sector,
at the fore end of Drum 1, where rung 1 builds.

The operator, 2026-11-06: "I guess this is further incentivizing our need to get the whole
mothership laid out! Right now our game world is very haphazard with homestead and community
elements weirdly placed with some just floating in space. I was thinking we should do a wipe of
the entire homestead and start fresh to make sure we get all the weird artifacts."

## 0. In short

- **The ship.** The UHS Mothership is a straight, non-rotating SPINE with spinning DRUMS strung
  along it, a BOW where craft dock and a STERN for the drive. Drum 1, the first drum, is the one
  the operator sized himself: 250 m radius, 2 km long, 1 g at the floor at 1.89 rpm (2026-09-15),
  a plain cylinder (decided 2026-11-06). Everything walkable in this plan is inside Drum 1, at real
  scale. The rest of the ship is data, drawn from outside and counted, not walked (section 3).
- **Sectors of 500** (decided 2026-11-07, 3.15). Every drum is built from sectors of 500 people:
  three neighbourhoods of about 167 around their mess halls, homes of about 5, clusters of 15 and
  blocks of 50 inside them (the research: 500 is the circle of acquaintances, 150 the group that
  works together; [2026-11-07-community-size.md](../reference/findings/2026-11-07-community-size.md)).
  A sector needs 0.206 km2 of drum floor: on Drum 1 a half ring 785 m around by 263 m along, every
  home 3.1 minutes from its mess hall and 6.2 from the sector's civic hall. Drum 1 holds about 15.
- **How big.** Measured bottom up by a SPACE PROGRAM (3.12): every kind of space with its area per
  person or per unit, from NASA's 1975 settlement study where it applies, totalled for a
  population by `node scripts/space-program.js`. At the default mix (8% of residents homesteaders)
  and with the ship's farms growing the comfortable garden for everyone they feed, 100 people use
  2% of Drum 1's floor, 1,000 use 13%, and 10,000 need 128%: Drum 1 holds 7,781. Homesteads take
  51% of the drum for 8% of the people.
- **A billion and ten billion** (his aim, 2026-11-07; 3.16) are a FLEET, not one ship: 2 million
  and 20 million sectors, in about 204 and 2,033 ships of 332 km, each 246 drums of 1 km radius
  and 4.9 million people. One spine for them all would be 67,500 km long and could not carry its
  own manoeuvring loads. About 5,220 t a person (most of it the 10 t/m2 shield chosen for 5 mSv a
  year, 1.5), about two millionths of the asteroid belt for a billion.
  The game never builds more than the sector you stand in; everything else streams as data.
- **Where people live.** Homes stand on the drum floor at 1 g, grouped in neighbourhoods of a
  few hundred people around a node (a commons with a mess hall, a recreation room, washrooms, a
  garden and a market row), every home within 5 minutes' walk of it. One homestead per plot, never
  stacked; apartments stack inside their own blocks around a planted atrium (decided 2026-11-06,
  3.6). Industry, the reactors, storage and hangars are in the spine, behind the bearing that
  separates it from the drum (sections 3.4 to 3.6).
- **Crew health first** (the operator, 2026-11-06: "We want to maximize crewmember health."):
  rules a layout is checked against, from dated WHO and NASA sources: 30 dB in a bedroom at night,
  40 dB outside, daylight and dark on a shared day, green space within 300 m, no crowding, every
  home step-free (3.13).
- **Arrival.** A transport docks in Hangar Bay 1 in the bow; the receptionist's desk is in the
  Arrivals hall beside it; a lift runs down the fore end cap to the Fore Landing on the drum
  floor, where General Services (barracks, public respawner, medbay) stands next to the lift
  foot; then the walk or the rail to your home (section 3.7).
- **The wipe.** Remove the whole ship file's layout (14 districts, First Street, the South
  Commons, the Commons and its old-home walls, 28 plots), the wild nodes and fixed quest places
  that were authored for a planet, the code that places the hologram and the guest's home, six
  orphaned data files and the relay's Pioneer upgrade. Keep the homestead and cabin designs
  (they are good and tested), the machines, the starts, and the operator's character (section 6).
  Districts out and wild nodes out are decided (2026-11-06): aboard, ore comes from distant
  asteroids by drone ([drone-mining.md](drone-mining.md)).
- **Rung 1.** Neighbourhood 1 of Sector 1 on Drum 1's floor, still flat: a continuous ground under
  everything (no more void), 12 homestead plots and a 16-cabin block side by side (decided
  2026-11-06) around a rebuilt commons whose garden sits in its own room, every placement in
  data, and the new gates (nothing stands in the void, no machine crosses a wall, every light
  inside its body, every frame named), each seen red against today's data first (section 6.4).
  Rung 2 rolls that floor into the drum.
- **The drive.** The cruise between the stars is a JUMP, with no thrust while travelling (his
  answer, 2026-11-07; a game fiction, said so in the game). Thrust is for manoeuvring only, along
  the ship's axis, routinely at most 0.02 g ("down" leans 1.15 degrees, flatter than the cross
  slope a walkway may have) and never above 0.05 g (2.86 degrees, the steepest floor that is not
  yet a ramp). So the floors stay fixed: no gimbals, no tilting decks (3.14). Between manoeuvres
  the spine is weightless and the drum a plain 1 g.

## 1. What is decided (the operator's words, dated)

| Date | Decision | Source |
|---|---|---|
| 2026-06-29 | The mothership needs a laid-out superstructure: "figure out how we lay out the superstructure of the mothership -- how to lay out rail lines, elevators, teleporters, hangar bays, mech bays, cargo tunnels, storage areas, massive industrial factory areas. We'll want to figure out how to build the public meeting zone / the shopping mall." | [mothership-superstructure.md](mothership-superstructure.md):7-10 |
| 2026-07-01 | Noise travels through the frame: "if the industrial area is too close, no one can sleep, the noise comes through the steel frame of the ship." | mothership-superstructure.md:55-56 |
| 2026-07-01 | The population is an aggregate, not rendered people: "we don't literally have to render everything and do physics for everything, most things are just simple calculations like going to x to spend x time then go to do this for x time, at any given time everything is being done by a certain percentage of people." | mothership-superstructure.md, open questions |
| 2026-09-15 | Two habitat kinds: a SPINE ("skyscraper like blocks that contain everything necessary to survive, like industry, residential, research, in a rough cube shape (or configurable)") and a DRUM ("the inside of a spinning drum where the gardens are visible from the inside... allows for gravity while not accelerating"). | [habitat-generation.md](habitat-generation.md):3-13 |
| 2026-09-15 | **First drum: 250 m radius, 2 km long.** "The smallest comfortable makes perfect sense." | habitat-generation.md:20-23 |
| 2026-09-15 | "The whole point of doing the drum/ring shape is to have the spin and gravity. If the spin were to stop then I'd like gravity to go away." | habitat-generation.md:27-29 |
| 2026-09-15 | The spine has thrust gravity: "the most common state of the fleet is going to be in the perpetually forward motion going between stars so it makes sense the spine has gravity." (Its premise is changed by the 2026-11-07 jump: a ship that jumps does not thrust, so its spine is weightless; 3.3, 3.14.) | habitat-generation.md:62-67; [gravity-and-movement.md](gravity-and-movement.md):3-11 |
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
| 2026-11-03 | "Agreed" to: small craft jump, the motherships never do and cruise under thrust (the motherships' part changed by his 2026-11-07 answer, section 1.2); weapons for things that are not people, sport by consent in the Arena. | game-setting-and-intro.md:1099-1116 |
| 2026-11-06 | The ship's hum: "I was also thinking we could incorporate a subtle brown noise that acts like the hum of the engines and the resonance of everything?" The orchestrator's journal records the decision as a hum "tuned to the music's key and louder near the engines". | [music.md](music.md):37-38; data/coordination/orchestrator_state.json, 2026-11-06 evening |
| 2026-11-06 | "do a wipe of the entire homestead and start fresh" (quoted above), after reading a plan. | this task's brief |
| 2026-11-06 | His answers to this plan's questions (section 7) | section 1.1 below |
| 2026-11-07 | His second answers: sectors of 500, 1 and 10 billion, a cruise without thrust, one level of farms, private craft, drones, the garden wipe | section 1.2 below |

### 1.1 Decided 2026-11-06 (night): his answers to section 7

Quoted exactly as the orchestrator relayed them to this plan's second pass (the journal,
data/coordination/orchestrator_state.json, summarises them in its 2026-11-06 night entry). Where
he asked a question or did not answer, the question stays open in section 7 and its
recommendation stands.

| Question | His words, 2026-11-06 | What the plan does now |
|---|---|---|
| 1. How big is the ship? | "How do we end up measuring the total size of the ship? Do we need to plot the size for each home, hangar, engineering bay, industrial zone, reactor room, bridge, storage areas, transportation like railways and elevetors, etc?" | Answered with a space program (3.12): yes, every KIND of space gets its area per person or per unit, and the ship's size follows from the population. The population to write down is still his (question 1 stays open). |
| 2. Cylinder or hyperboloid? | "I believe this is the best route." | **Decided: Drum 1 (then called Sector 1) is a plain cylinder**, 250 m x 2 km. The hyperboloid stays a candidate for the ship's outer form or a later sector. |
| 3. Where the drum's light comes from | On the sunline: "I look forward to seeing it." | **The sunline is the drum's light**: the spine's light strip on the ship's day cycle (3.2, 3.13). Piped sunlight or lamps follows where the fleet is, still open as game-setting-and-intro.md question 2. |
| 4. One deck or many for homes? | "We'll start with one. We might change depending on how the game develops. One floor is easier. However we still want to allow multiple stories so that we can get multiple floors working, just not multiple homesteads stacked vertically. I wouldn't mind apartments and other tight packed arrangements being multiple stories. That way we could have like an atrium with trees and a common gathering area." | **Decided: one deck, the drum floor.** One homestead per plot, never stacked; a home may have storeys of its own (the homestead's upper floor); apartments and other tight-packed homes may be multi-storey buildings around an atrium with trees and a common gathering area. The apartment block template is data (3.6). |
| The garden | "Our goal is a minimalist survival garden that provides all the nutrients it possibly can." | **The garden's goal** (3.10). The garden is designed separately by the homestead engineer (`docs/design/survival-garden.md`, in progress). He did not speak to wiping his own planting, so question 6 stays open. |
| 7. The old walls in the Commons | "Let's try it." | **Decided**: the eight-material showroom ring moves to an exhibit room of its own in N1's commons; the 15-wall room block is retired (6.1, 6.2). |
| 8. Ore nodes aboard? | "Correct. Resources come from distant asteroids we send the drones to collect." And: "We can allow players to pilot drones and add a drone mining game for mining but, I don't know how we should do it. Maybe a cargo capacity and fuel limit." | **Decided: no wild nodes aboard**; ore comes from distant asteroids by drone. The drone mining game, automated by default with an optional piloted mode, limited by propellant and hold, is designed in [drone-mining.md](drone-mining.md). |
| 9. The districts | "Sounds good. We want to get close to what a real-life spaceship of this design might be like." | **Decided**: all fourteen out now, each back as a real walkable place in its rung. **Realism is the standard** for the whole ship. |
| 10. Cruise thrust | He asked: "So this means while on the ship we'd be in 5% earth gravity?" | Answered in 3.14 (no: 1 g on the drum floor, leaning 2.86 degrees toward the stern while burning; 0.05 g in the spine). Still open, now with a sharper question: does the ship burn all the time or coast between burns? |
| 11. How far homes sit from loud places | "We want to maximize crewmember health." | **The direction is his**: the strictest health-based value wherever sources differ. The rules come from a dated finding (3.13, [2026-11-06-noise-and-sleep.md](../reference/findings/2026-11-06-noise-and-sleep.md)). |
| 12. Neighbourhood 1's mix | "Ideally both." | **Decided**: homesteads and the cabin block side by side in N1 (6.4). |

Questions 5 (what of today's home to keep) and 6 (his garden and save) had no answer; their
recommendations stand.

### 1.2 Decided 2026-11-07: his second answers

Quoted exactly as the orchestrator relayed them (the journal, data/coordination/orchestrator_state.json,
records them in its 2026-11-07 entry). Where he asked a question, the plan answers it and the
question stays open until he reads the answer.

| Question | His words, 2026-11-07 | What the plan does now |
|---|---|---|
| 1. How many live aboard? | "Let's aim for 1 billion and 10 billion." | **Decided: the aims are 1 billion and 10 billion.** The honest answer is a fleet (3.16): about 204 ships for a billion and 2,033 for ten billion, each 332 km long with 246 drums of 1 km radius and 4.9 million people. One spine cannot hold them. |
| 1. The sector | "Let's start with a sector for 500 people. I've heard that communities of 500 people is about the upper limit for one massive group to work together. Can you research that number?" | **Decided: a sector is 500 people.** Researched ([2026-11-07-community-size.md](../reference/findings/2026-11-07-community-size.md)): 500 is the circle of acquaintances and the size past which communities need a formal structure; the group that works together is about 150. So 500 is the sector's cap, with three neighbourhoods of about 167 inside it (3.15). The drum he sized is now called Drum 1; Sector 1 is its first sector. |
| 10. Cruise thrust | "We can introduce some sort of mechanic that requires no thrusting. Like a teleportation tech requires no thrusting." | **Decided: the cruise is a jump with no thrust** (3.14), named in the game as made up. Thrust is for manoeuvring only. This changes the 2026-11-03 rule that the motherships never jump (game-setting-and-intro.md 10.1); question 10b asks him to confirm it. |
| The lean | "I'm wondering how we'd design a drum design that has subsections that are able to adjust to either one or two axis. Gyroscopes start add a lot of excess structure." | Answered in 3.14: five options compared; recommended: fixed floors and a thrust cap (0.02 g routine, 0.05 g at most), because the lean is then no steeper than floors people walk every day, with no structure added. One axis is all any option would ever need. |
| 14. Farms | "Let's start with one but, allow for two or more." | **Decided: one level of farms**, with the data allowing two or more (`[farms] levels` in data/ship/space_program.toml; `--farm-levels 2` lets Drum 1 hold 9,031 instead of 7,781). |
| 15. Private craft | "cradle being the revive mechanic?" | **No: a "cradle" was a parking place for one private craft.** The word misled, so it is now **parking spot** everywhere a person reads it (this plan, the space program's `craft_parking`, the ship file's hangar filler) and in the code (`mesh_kind: "parking_spot"`). Reviving people is respawn-and-revival.md's, whose words (pod, respawner) do not overlap. The question of how many craft (one for every 20 people) stays open with its recommendation. |
| 16. Drones | "Maybe not jump between stars but, at least jump to discovered nearby asteroids/comets/whatever." | **Decided: drones jump only to discovered nearby bodies**, never between stars (drone-mining.md, sections 2 and 4). Where the drone is kept was not answered; the plan keeps its recommendation, the bow's drone bay (section 8 there). |
| 6. The garden | "Wipe it." | **Decided: his garden is wiped** with the rest. The rest of question 6 (his character, car and save) and question 5 stay open with their recommendations. |

The garden's own answers (the comfortable garden as the start, the mushroom room, the crop data
fixes, phosphorus) are in [survival-garden.md](survival-garden.md), section 13.

Standing rules that shape the plan (CLAUDE.md): infinite-of-x (anything that exists more than
once is data); no backwards compatibility before v1.0, except the version-tagged blueprint schemas
that must keep loading old player saves; realistic-first (build the 2030 technique as a rung of
the real thing); every deep system has a full-realism and a simplified mode; compute and bandwidth
are a hard budget; never trim dev or teaching tools.

### 1.3 His thoughts on home styles and sizes (2026-11-12, open)

Before the wipe he raised the sizes of the hangar and the places beyond the home, and the home's style. Verbatim: "I'm a bit mixed about the current house design. I kinda feel like we should go back to the fibonacci styled home. I'm not really sure though. We also gotta figure out how big the other places should be not the player home." Then, after top-down drafts of 21 x 13, 34 x 21 and 55 x 34 Fibonacci homes: "The fibonacci idea is great for one person but, as you said when multiple people are involved it kinda breaks. Also, it gets huge fast. Initially I chose 34x34 for the garden because, that's what I read is supposedly the amount of land required to have a full garden for food year round for one person. However we're in a spaceship and space is limited. I figure from that everything below makes sense for a home and everything above the garden makes sense for utilitarian stuff. Like the garage and everything after is like ranches of ever increasing size. However I'm not sure how that'd fit on a spaceship well."

**Proposal** (the coordinator's, not decided): keep Fibonacci as the scale ladder of the COMMUNITY rather than of one house: squares of 13 m and below are your home; 21 and 34 are a cluster's shared workshop and garden (about 15 people); 55 and 89 a neighbourhood's workshop hall and park or farm (about 167); 144 the sector's civic heart (500). Ranch-scale land belongs to the ship's farm band or to planets, not to homes. Home styles to draft and compare: courtyard house, row houses on the drum's curve, a cluster of homes around shared space, a modular grid, a honeycomb. The planning sheet shows why: at today's mix 16 homesteads (40 people) take about 103,000 m2 of drum floor and the other 460 people about 11,000 m2 (`node scripts/space-program.js`, 2026-11-12). Open: his choice of style, and every size checked against real references (the research pass queued for after the reset, including the hangar sized from a defined transport and the garden from sourced per-person food-growing areas for soil and for controlled-environment farms).

**His second round (2026-11-12, directions, not yet decisions):**

- **Livestock, with good lives:** "The point of the ranches is I was wanting a place to grow livestock. Plants have their place but, livestock offer an additional layer of resiliency that plants alone can't offer. How might we include animals but, provide them good enjoyable living conditions. We can't force them to live in tiny cages."
- **Shared homes:** "I like this idea for the sake of people wanting to play together but, share one garden. Maybe only 1 of the residents is wanting to tend to the garden. I guess this is kinda like dormitories?"
- **A real-world module:** "I kinda like the idea of tatami mats. That'd be immediately applicable to real life for an entire nation. Any design we come up from that is then \"to code\" with them." (Whether a module makes a design "to code" is a building-regulation question: research it into a dated finding before any claim, CLAUDE.md.)
- **Row houses and trees:** "Honestly the row houses makes tons of sense. Also would make designing an orchard real easy. That's another big thing we need to account for, trees."
- **Power shapes the ship:** "We also need to figure out providing power for all the residence. That might help us design the shape and scale. Like I think fission nuclear reactors make great sense. Also we could have fusion reactors but, their benefit is different. Obviously we have solar power. If we incorporate flowing rivers of water we'd have hydro power too. About the only one I can't think of incorporating is wind power unless we like line the walls of the hangar so that as spaceship use their thrusters they generate wind for the wind turbines?"
- **The Fibonacci community and the road home:** "I'm kinda liking this idea." and "Essentially a consistent yellow brick road that's easy to know whether you're headed home or outwards. I guess we'd want to go homes then gardens/parks then industrial then hangars on the outside? Also, for like the ranch area and public spaces... The ranch/gardens/parks and maybe the homes are present on the inside of the ring able to see the other side of the drum. Then like the hangar and other like utilitarian stuff is on the outside to act as a layer of armor between the garden floors vs the outer walls facing the vacuum of space."

**Coordinator's answers, proposals for his decision:** livestock as pasture in the farm band (rotational paddocks, chickens and goats in the orchards, fish in the rivers, bees for pollination), each kind's space per animal from real welfare standards (to research); co-housing is private homes (own kitchen and bath) plus shared spaces, not dormitories, and roles split by interest; modules as data (tatami, metric 600/1200 mm, imperial 4 ft) so a design maps to real materials, with building-code minimums researched per country before "to code" is claimed; row houses with orchard strips behind them; power: fission now and fusion as the upgrade, solar only near a star, rivers as pumped-hydro storage and radiation shielding rather than a net source, wind inside a closed ship costs more than it gives (fans driven by the ship's own power) and thruster wash inside a hangar is unsafe, so no wind; the radial layering he described (homes, parks, orchards and paddocks on the inner floor seeing across the drum; industry, storage and water as armour under the floor against the hull), with the hangar staying at the bow on the spin axis because Drum 1's rim moves at about 50 m/s (0.198 rad/s x 250 m) and craft cannot dock there.

**His third round (2026-11-12, questions for the research pass):**

- **A closed-loop ecosystem:** "There's a very high chance we won't have pests. HOWEVER we may want to intentionally add pests. Like bees are pollinators so we don't manually have to do so or use robots. Bees are much cheaper than robots. Then if we have soil, it makes sense to have worms and what not. How do we do this right so we create a closed loop ecosystem that only receives raw biomatterless space rock powders."
- **A housing tier ladder:** "I think we need to come up with a tier system. Like barracks would be the most extreme version of shared housing. Everyone has excess to everything in giant studio like rooms. Everyone tends to the garden. Then at the other end of the spectrum is like 1 person and the minimum necessary for them to self-sustaining with plants/animals."
- **Building the drum:** "How would we construct the drum? What materials would we use? Let's assume we're using steel. How thick should it be to support the super structure and all the other elements like engineering, hangars, etc. Also, what's other materials we might end up using instead like titanium. What other materials make sense? Also, We'd likely only have furnaces for a certain size. What would the dimensions of the metal sheets be? Would we actually try bending the sheets or would we create them flat then weld them. I guess these sheets also act as the polygons for the structure of the vessel."
- **Distributed reactors and a radiation catastrophe:** "That means we have to account for hydrogen usage and uranium (or whatever others). We may want to consider designing a housing chunk for placing many fusion/fission reactors across the ship. Not just a centralized one. What size reactor would we want. What's the minimum range we'd want homes to be? Gardens? Let's say something fails and a bunch of radioactive material leaks from the reactor chamber into the main room absolutely flooding it with radiation. How do we shield humans/animals/plants for that catastrophe?"
- **Trees need wind:** "We also need to consider the fact trees need wind to grow strong enough roots to support them when they reach peak height." (Trees grown without wind form weak wood; Biosphere 2 reported it. So the orchards and parks need moving air on purpose, from the ship's own ventilation or dedicated fans, as a growing condition rather than a power source; the research pass sizes it.)
- **Materials and the water shield:** "How do basalt fiber and carbon fiber compare? What's basalt fiber used for today?" and "What if the radiation shield was a layer of water in small controlled layered pockets on the outside hull of the spaceships? Like in a foam like material instead of free flowing in an open pool." (Proposal: water in sealed cells on a NON-spinning outer shell, so its weight adds no load to the drum; one puncture loses one cell; cells must survive freezing on the shadowed side or be heated; the research pass sizes the depth against cosmic rays.)
- **Solar surfaces:** "I've seen articles about transparent solar panel windows. We can say all windows are solar panels. The same for all painted surfaces. Nearly every square inch could functionally be a solar panel of some efficiency percentage."

Queued for the research pass after the 2026-11-13 reset, each answer sourced and dated (CLAUDE.md): the closed loop (pollinators, decomposers, nitrogen and carbon sources when the only input is rock powder; Biosphere 2, MELiSSA and BIOS-3 as references), the housing tiers, the drum's structure (hoop stress from spin, air pressure, floor and shielding loads; steel, titanium, aluminium, basalt and carbon fibre; plate sizes and whether to roll or facet them), reactor sizing and siting (power per person including farm lighting, small modular and micro reactors, fuel use, vault shielding and the catastrophe case), and photovoltaic surfaces (efficiency of transparent and painted PV, and that panels inside the ship only recapture the ship's own lamp light).

**Answered 2026-11-13: [the mothership sizes research](../reference/research/2026-11-13-mothership-sizes/README.md)**, every number sourced, dated and quoted, with the operator's decisions listed at its end. The headlines: the hangar as four bays from defined transport classes (an S-92-sized shuttle 1,140 m2, an ATR 72-sized ferry 1,520 m2, a C-130 class freighter 3,290 m2, a heavy lift for one 100-tonne-plus vehicle 5,950 m2 of deck), now in the space program; housing tiers from barracks (6.7 m2 of sleeping floor) to a homestead (372 m2 of soil beds or 40 to 65 m2 under lamps per person to feed one), and a 34 x 55 m household plot that would let Drum 1 hold 10,875; livestock space from welfare standards (a pasture-raised hen 10 m2); rock powder supplies no nitrogen or carbon, which come from carbonaceous asteroids and legumes; the drum's steel 14 cm for air alone and 17.5 cm with soil at a pressure-vessel allowable; a **correction**: 4.5 t/m2 of shield gives about 55 mSv a year, and 7 t/m2 of water gives 16 (he chose 5 mSv, about 10 t/m2: 1.5, carried into 3.16); three RITM-200N-class reactors for a full Drum 1 in unpressurised vaults; basalt fibre not for clothing, linen at about 80 m2 of flax a person; a colony module of at least 2,000 and planned at 5,000 people (1.4); burning hydrogen is a water loop, not a source, and rain from the spine lands tens to hundreds of metres to one side (1.4).

### 1.4 His answers on the world aboard (2026-11-12, directions)

Verbatim, each to one of the coordinator's questions:

- **Heavy transports:** "What do you think about having ships large enough to transport those giant kamatsu trucks as big as a office building used for the biggest mining operations? We're wanting to support battlemech like vehicles the size of the atlas or direwolf. They need a transport ship large enough for 100 tons or more per cargoed vehicle."
- **Livestock in confinement:** "If we need super confined cages I've seen things about testing VR and omnidirectional treadmills for livestock."
- **Housing showcases:** "Could we have sections on the ship that showcase each one? Like hop on the tram to go to the housing style X residential sector. Go here to view barracks. Etc."
- **Craft classes and mooring:** "I like the idea of single crew/pilot ships up to 100 crew or more. I imagine a C130 stratofortress transport spaceship has its purpose. We'll also want to include mooring locations so people with weirdly shaped or too large of ships can still dock."
- **A day aboard:** "What about leisurely hanging out among the gardens where it is healthiest for the human body? I figure hanging out, playing loud music dancing about, playing physical games, paintball, watching movies, walking around, running, etc. Other options include conducting repairs, upgrades, transporting goods themselves, going on expeditions to explore alien stations to find supplies/gear. What do you think?"
- **Weather:** "Weather makes sense because, otherwise we're building drip lines everywhere. Or at least something like that. Well, if we burn hydrogen we generate water, right? What if we incorporate that some how? Or should it just be emitted from sprinklers?"
- **Town names:** "So I guess we need to figure out how we want to name the various \"towns\" aboard a mothership."
- **Vacancy:** "Ideally the mothership has enough vacancy players can always choose whatever they want. We'd have to get ten billion users/crew on the mothership to use up all the home potential."
- **The colony module:** "I figure the colony module would be a single 'vehicle' or space city or something that supports an entire population of humans infinitely without worrying about inbreeding comfortably."
- **Danger:** "The first thing that comes to mind is meteor strikes. Part of the problem of designing an interplanetary spaceship is that it's designed to be so resilient that ideally nothing can harm it. Other options could be alien invasion either from our dimension or others (Warhammer 40k uses something like the geller field to protect its ships in the warp from demon invasions.) Maybe a container of some explosive material explodes damaging the ship. Zombies."

**His answers to the notes, the same day:** on our own walkers, "Of course. Only what we can legally use."; on town names, "We can have a vote that only the residents of the housing sector can vote on. We could choose like a default name. Like Barracks Housing 455B or whatever." (so every housing sector starts with a plain default name of its style and number, and its residents alone may rename it by a signed poll); on danger, "Agreed. Real skills first then we can have fun with fantasy stuff."

**Coordinator's notes (proposals):** heavy-lift transports carry 100-tonne-plus walkers and mining haul trucks (our own designs: BattleTech's Atlas and Dire Wolf and their look belong to their owners, so the game sizes a class, not their mechs); craft from single-seat to 100-plus crew, a C-130-class transport (the Hercules; the B-52 is the Stratofortress) and moorings outside the bays for odd or oversized ships; one sector per housing style on the tram line as a showcase and a choice; livestock get real space and enrichment from the farm band (the research pass sizes it) rather than VR in cages; leisure in the gardens, sport, music and film beside repairs, upgrades, hauling and expeditions to alien stations; engineered rain from the spine on a schedule (fuel cells make water from hydrogen and oxygen, but the oxygen comes from splitting water, so it is a loop, not a new source); towns named by a convention he picks, possibly through the long-form poll; capacity far beyond any population so players always choose; the colony module sized by real minimum-viable-population research plus gene banks; threats real first (meteoroids, solar storms, fire, decompression, reactor faults, disease, crop failure) and the gates as the opening where anything from elsewhere could come through, shielded in transit like his Geller-field example; zombies as an outbreak scenario or a mode.

### 1.5 Decided 2026-11-13: the research pass's twelve

The operator, to the twelve decisions at the end of [the sizes research](../reference/research/2026-11-13-mothership-sizes/README.md), with the coordinator's recommendations (shield: 5 mSv a year): "I'm happy with your recommendations for all 12 things. Let's go with those." So:

1. **Housing:** all five tiers (barracks, dormitory, co-housing cluster, row house, homestead) as home kinds, barracks also as General Services, and one showcase sector per style on the tram line (1.4).
2. **Homestead plot:** 34 x 55 m per household (a Fibonacci rectangle: its squares 34, 21, 13, 8, 5, 3, 2, 1, 1 are the garden, the workshop and vehicle bay, and the home's rooms, the coordinator's draft C of 2026-11-12), plus a 21 x 34 m one-person homestead. **Redirected the next day (2026-11-13):** "What if we instead go the route of figuring out which rooms we actually need. Like they're unique enough to justify their own room. Like the kitchen or bedroom." So the homes are designed from a ROOM PROGRAM (each room justified by a function needing walls of its own, sized from the standards in the sizes research), laid out on the decided plots; Fibonacci stays only where it genuinely fits, not as a rule the rooms obey: [the home room program](home-room-program.md), phase 1 proposed 2026-11-13 with two drafts and seven questions.
3. **Transport classes:** S shuttle (19 seats), M ferry (72), C freighter (C-130J size), H heavy lift (one 100-tonne-plus vehicle a bay); Starship-sized craft later.
4. **Bays:** one S per 2,000 people, one M per 5,000, one C and one H per 10,000, at least one of each; four mooring masts.
5. **Private craft:** an R44-size four-seater, one for every 20 people. His addition the same day: "We could offer an upgradeable hangar spot for users. Start is a small craft and then upgrading to like 12 and then so forth. That way a player could earn the right to lead a 100 man party on a PvE expedition." So a player's hangar spot grows with what they earn: the four-seater first, then a 12-seat craft, and upward to craft for a 100-person PvE expedition.
6. **Reactors:** three RITM-200N-class units for Drum 1 (any two carry its 84 MW), two eVinci-class for a lone first sector; fusion later.
7. **The herd:** hens and bees in the orchards, fish, rabbits, a few goats, at welfare-standard space.
8. **The shield:** 5 mSv a year, about 10 t/m2 of water in sealed cells on a non-spinning outer shell.
   Same day, after the active-shielding research ([2026-11-13-active-shielding](../reference/research/2026-11-13-active-shielding/README.md)):
   the floor, soil and stores count toward the 5 mSv once a transport calculation confirms it ("Makes
   sense to include it"; the data stays at 10 t/m2 until then). He asked why not foam cells beyond the
   thickness, whether one Earth-like field generated from the ship's centre could replace a magnet on
   the shell ("kind of like the warp bubble from Star Trek"), for the OLTARIS link, and what a slag
   layer means; answered in the journal's entry for the day. Decided the same evening: the shell's
   water sits in **valved compartments** of a few cubic metres (a strike drains one, the water stays
   pumpable as the ship's reserve and every tank can be inspected), not foam: "Makes the most sense
   for long term longevity and resiliency." And slag is not a later rung: "Mining technically already
   exists. Players send out drones to mine." So the drones' ore, smelted, leaves slag that is packed
   onto the hull outside the water (rock outside, water inside to soak up the secondary neutrons), a
   loop in which a player's mining thickens the ship's armour. On the mass (he was "mixed": "how
   feasible is it? I imagine the extra mass drastically slows the mothership down. Would we still be
   able to turn?"): slag replacing part of the water adds about 9% to Drum 1 (turns about 5% slower);
   slag on top of the full water adds about a third; the cruise is a jump and the drums turn in pairs,
   so mass costs only manoeuvres (coordinator's estimates). Decided: armour against agility is **a
   player vote per ship** ("Should be a player vote per ship"), and slag is to be justified as an
   endless resource sink by its uses ("We'll have to figure out the various ways we can use the slag.
   That way we can justify it as an infinite resource sink."): armour, reaction mass for a mass
   driver, and the rest a research pass lists.
   **See also: [slag.md](slag.md)** (2026-11-13): every use ranked by how much it absorbs, the
   loops, the vote's numbers (1 t/m2 over Drum 1 adds about 7% to its mass, 4% to its turn time,
   and takes the dose from 5 mSv to about 3.3 to 4.0), the rungs and eight questions; sourced in
   [2026-11-13-slag.md](../reference/findings/2026-11-13-slag.md).
9. **Drum material:** steel for Drum 1, plates rolled to the curve; basalt composite studied for later drums.
10. **Cloth:** a row in the planning sheet, 4 kg a person a year, about 40 m2 of flax.
11. **The colony module:** 5,000 people with a gene bank.
12. **Trees:** a steady breeze plus gusts, proven in an orchard test.

## 2. Where the pieces disagree, and how this plan settles them

| Conflict | The sources | This plan (Proposal unless quoted above) |
|---|---|---|
| How big is the ship? | A 250 m x 2 km first drum (decided); "a population of over a billion" (decided wording); "Let's aim for 1 billion and 10 billion." (decided 2026-11-07); docs/game/humanity_one.md's 500 km x 100 km ship for 10 billion (vision, never decided); today's 1.2 km flat slab (built) | Measured by the space program (3.12): Drum 1 holds about 7,800 at the default mix, about 15 sectors of 500. A billion and ten billion are a fleet of about 204 and 2,033 ships of 1 km drums (3.16); humanity_one.md's single hull fails the spine and spin-stability limits. Drum 1 is the decided drum and the only walkable one. |
| Cylinder or hyperboloid? | The operator's hyperboloid line (2026-09-15); the 150 m waist and its gravity table are an agent's elaboration (gravity-and-movement.md:430-448) | **Decided 2026-11-06: a plain cylinder** ("I believe this is the best route."). At those numbers a hyperboloid's floor slopes 9.1 degrees along the axis at the flared ends; it stays a candidate for the ship's outer form. |
| At Earth or between the stars? | The ship orbits over Silverdale today (data/stations/home.ron); the lore says between the stars (game-setting-and-intro.md question 2; leaving-the-ship.md question 1) | Inside a drum the light comes from the spine's sunline either way: piped sunlight near a star, lamps between stars. Question 3. |
| Where do you wake? | Today: your front door (the home's `spawn`); the 2026-11-02 brief: home, or a place you choose, or General Services; GSI: a pod beside the bedroom | Respawn places are data rows (section 3.7); a home's pod waits for respawn-and-revival.md's answers. |
| The old intro | docs/game/intro_sequence.md: private landing ports, quarters on a habitat ring | Superseded by the 2026-11-02 brief; game-setting-and-intro.md question 13 asks to retire it. |
| One flat deck or a drum? | Everything built stands at y 0; every decision says drum | Rung 1 lays out the drum floor unrolled (flat, in drum coordinates); rung 2 rolls it. One layout, one set of coordinates, a curvature switched on. |

## 3. The ship at real scale

### 3.1 The mothership: a bow, sectors on a spine, a stern

```
 SIDE VIEW (not to scale between parts; along the axis, the bow faces the direction of travel)

  gateway          BOW                 DRUM 1                DRUM 2 ... DRUM N           STERN
  (a point     +----------+   +=========================+   +======== ... ========+   +-----------+
   in space)   | Hangar   |   | 250 m radius, 2,000 m,  |   | 1 km drums (3.16),  |   | drive,    |
  transports   | Bay 1,   |===| 1.89 rpm; ~15 SECTORS   |===| 40 sectors each,    |===| reactors, |
   arrive -->  | Arrivals,|   | of 500 (Sector 1 first) |   | counted, drawn      |   | radiators |
               | freight  |   |  <- SPINE (non-rot.) -> |   | from outside        |   |           |
               +----------+   +=========================+   +======== ... ========+   +-----------+
                 weightless   ^ fore end cap  aft end cap ^                            the hum is
                 between      (spoke lifts)  (risers,                                  loudest here
                 manoeuvres                   freight lifts)
```

| Part | Size | Gravity | Holds | Status |
|---|---|---|---|---|
| Bow | Proposal: 120 m across, 300 m long, non-rotating | weightless, except a light push during manoeuvres (3.14) | Hangar Bay 1 and further bays, the Arrivals hall and the desk, mech bays, the drone bay and ore intake, freight intake | Proposal |
| Drum 1 | **250 m radius, 2,000 m long, a plain cylinder** (decided 2026-09-15 and 2026-11-06) | 1.00 g at the floor (0.988 g at 3 m up, 0.976 g at 6 m) | about 15 sectors of 500 (3.15), the Concourse, General Services | Proposal for its contents; its length against spin stability is question 1b |
| Drum 1's spine segment | Proposal: 60 m across, 2,000 m long, decks across the axis every 5 m (10 m for industry) | weightless, a light push during manoeuvres | drum engineering (reactors, air, water, waste), industry, storage, the freight rail | Proposal |
| End caps | discs of 250 m radius at each end of the drum | 0 at the axis to 1 g at the rim | spoke lifts (4 a cap), bearings, utility risers | Proposal |
| Further drums | data rows: 1 km drums of 40 sectors each, in ships of 246 drums (3.16) | | the rest of the population, as an aggregate | Proposal |
| Stern | Proposal: data; the main radiators, the reactors that charge the jump | weightless | the ship's loudest machinery | Proposal |

**The arithmetic of size** is now the space program's (3.12) and its scale study (3.16). Drum 1
holds about 7,800 people at the default mix of homes, 11,500 with no homesteads and 1,200 if
everyone is a homesteader. A billion people need 2 million sectors; at Drum 1's radius that is a
million drums. So the ship grows in drums of a larger radius (1 km, 40 sectors each) and the
population in ships: about 204 ships of 246 drums for a billion, 2,033 for ten billion (3.16).
The aggregate tier of crowd-simulation.md counts them at no per-frame cost.

### 3.2 Drum 1

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
- **During a manoeuvre the floor leans**: 1.15 degrees at the routine 0.02 g, 2.86 degrees at the
  0.05 g cap, only while the thrusters push; the cruise is a jump with no thrust (decided
  2026-11-07). What that feels like, and why no floor needs to move, is 3.14.
- **Spin stability**: Drum 1 is 8 radii long, and a cylinder flying free spins stably only up to
  1.3 radii (325 m here; Kalpana One, a 250 m radius settlement, was cut to 325 m for this reason;
  [2026-11-07-drum-limits.md](../reference/findings/2026-11-07-drum-limits.md)). Held on its
  bearings around the spine, with the ship actively damped, it can still work; or its 2 km can be
  built as a row of short drums, each one ring of two sectors (3.15). Question 1b.
- **The sunline** (accepted 2026-11-06: "I look forward to seeing it.") lights the whole floor
  from the axis on one shared day: daylight bright enough at the eye by day, dim evenings, dark
  nights (the light rules of 3.13).
- **A flat home does not fit a 250 m floor.** A 55 m wide homestead built flat on the curve
  stands 1.51 m off the floor at its edges and its floor leans 6.3 degrees there against local
  gravity (computed). So homes are not placed flat on the drum: their designs stay authored flat in
  their own metres and are ROLLED with the drum, walls radial, floors following the curve (rung
  2). That is the same "one generator, a curvature parameter" idea habitat-generation.md:77-86
  proposes.

### 3.3 The spine, the bow and the stern

The spine does not rotate, so its only gravity is a manoeuvre's push, and the cruise is a jump
with no thrust (decided 2026-11-07): the spine is weightless nearly all the time (3.14). Its decks
are discs across the axis, stacked like a skyscraper lying along the axis, with handholds and
magnetic boots. Weightlessness is right for what habitat-generation.md:56-60 says a spine is for
(moving mass, big structures, docking), and wrong for daily life, so nobody lives there
(ship-homes-and-logistics.md section 2.8, Proposal). This changes the premise of the 2026-09-15
decision that the spine has gravity ("the most common state of the fleet is going to be in the
perpetually forward motion"): steady motion and the jump give no weight (3.14).

**Drum 1's spine segment** (Proposal), fore to aft:

| Segment (axial m) | What | Why there |
|---|---|---|
| 0 to 150 | Fore hub: the spoke lifts' tops, the passenger rail station | the lifts meet the axis at the end cap |
| 150 to 900 | Storage and the cargo tunnels (freight rail along the axis) | goods physically moved (decided); low gravity eases handling |
| 900 to 1,500 | Industry: fabrication, the factory district, machine shops | the noise rule: as far as possible from the fore landing's homes, isolated from the drum by the bearing |
| 1,500 to 1,850 | **Drum engineering**: a reactor pair of the game's class (two KLT-40S class, 60 MW net, closed-loop-test.md:64-66), switchgear, the air plant, water recovery, waste processing | near the aft end cap's utility risers; one pair feeds about 5,560 people at 10.8 kW each (computed) |
| 1,850 to 2,000 | Aft hub: freight lifts down to the Aft Landing, utility risers | |

**The bow** (Proposal): Hangar Bay 1 (today's district is 50 x 12 x 40 m; it becomes a walkable
zone with big doors, parking spots for craft and a launch lane, as [mothership-superstructure.md](mothership-superstructure.md)
M5 describes), more bays as arrivals grow (game-setting-and-intro.md:811), mech bays beside them,
the Arrivals hall with the receptionist's desk, freight intake and customs, and the drone bay
(drone-mining.md section 8). The transports dock here because docking "does not fight a rotating
frame" (habitat-generation.md:56-60). The manoeuvring thrusters that pull the ship sit here too
(3.16: a long spine is pulled, never pushed).

**The stern** (Proposal): the main radiators, the reactors that charge the jump, and attitude
thrusters. Radiators matter at this scale: 67 m2 of fin per person (closed-loop-test.md:84), about
0.34 km2 for 5,000 people, mounted on the non-rotating spine outside the drums.

### 3.4 Drum 1's floor, cut into sectors

Since 2026-11-07 the drum floor is cut into **sectors of 500** (3.15), each a self-contained
community with its own homes, mess halls, civic hall, workshops and farms. On Drum 1 a sector is a
half ring, 785 m around by 263 m along, so two sectors fill one ring of the drum. The drum's own
places (the Fore Landing, the greenbelt the sectors share, the Aft Landing) sit between the rings.

```
 DRUM 1'S FLOOR, UNROLLED (to scale: 1 character = about 33 m around, 50 m along)
 around (s) ->  0 ......................... 785 ......................... 1571 (= 0 again)
 along (a)
   0 +-----L1--------------------------L2-------------------------L3--------------L4-+
     | FORE LANDING: spoke-lift feet L1-L4, Arrivals foot (L1), General Services,    |
 120 | the Concourse (market, food hall, fleet board, memorial), rail station        |
     +---------------------------------------+---------------------------------------+
     | SECTOR 1 (rung 1 builds its N1)       | SECTOR 2                              |
 383 | three neighbourhoods, a civic hall,   | the same template                     |
     +---------------------------------------+---------------------------------------+
     | SECTOR 3                              | SECTOR 4                              |
 646 +---------------------------------------+---------------------------------------+
     | SECTOR 5                              | SECTOR 6                              |
 909 +-------------------------------------------------------------------------------+
     | GREENBELT shared by the drum's districts: park, the lake (water store and     |
1000 | thermal mass), orchards, the arena field, the mid-drum rail station           |
     +---------------------------------------+---------------------------------------+
     | SECTORS 7 to 12, three more rings of two (1,000 to 1,789)                     |
1789 +-------------------------------------------------------------------------------+
     | spare: a 13th ring, or the drum's district places (hospital, large market)   |
1880 +-------------------------------------------------------------------------------+
     | AFT LANDING: freight lifts, depot, food processing, recycling and compost,    |
2000 | the utility risers (the drum floor's noisiest band)                           |
     +-------------------------------------------------------------------------------+
```

| Band (a, m) | Area | Use | Rules it serves |
|---|---|---|---|
| 0 to 120 | 0.19 km2 | Fore Landing: Arrivals foot, General Services, Medical Bay, the Concourse | game-setting-and-intro.md:730-732 (General Services beside the Medical Bay and near Arrivals); mothership-superstructure.md M3 (the civic heart) |
| 120 to 909 | 1.24 km2 | Rings 1 to 3: sectors 1 to 6 (3,000 people) | each sector self-contained (3.15) |
| 909 to 1,000 | 0.14 km2 | Greenbelt and the drum's district places | at least 10% parks (Perry, section 2.5); every district of 1,500 shares larger places (the community-size finding) |
| 1,000 to 1,789 | 1.24 km2 | Rings 4 to 6: sectors 7 to 12 (3,000 people) | |
| 1,789 to 1,880 | 0.14 km2 | a spare ring's width, or the drum's district places | |
| 1,880 to 2,000 | 0.19 km2 | Aft Landing | freight and noise kept from homes (section 2.8) |

Laid out like this Drum 1 holds **12 sectors, 6,000 people**, with room for the places the drum
shares; the space program's area bound is about 15 (7,781 people) if the shared places pack
tighter. All Proposal. The bands are rows of a data file (section 4), not code. If Drum 1 is built
as a row of short drums (question 1b), each ring above is one drum of 263 m plus its end walls,
and the landings move into the end caps.

### 3.5 Neighbourhoods

A neighbourhood is about 167 people (a third of a sector; the research's layer of about 150 that
works together, split rather than grown past about 200) around one NODE. On Drum 1 its cell is
about 262 m around by 263 m along, a third of its sector's half ring (3.15); rung 1's N1 keeps the
393 m by 300 m cell below until rung 3 re-cuts it (Proposal):

- **The node, the neighbourhood commons:** a mess hall with its serving counter and food store, a
  recreation room, washrooms, a garden court (the commons garden in its own glass-roofed room), a
  market row, parcel lockers on the freight side, a clinic room and a Library room as those
  systems arrive (ship-homes-and-logistics.md section 2.5's node list).
- **Walking:** the farthest door is 345 m from the node along the streets in rung 1's N1 (4.1
  minutes at 1.4 m/s), inside the 5-minute rule that is already a test
  (`every_home_reaches_a_mess_hall_within_five_minutes`).
- **Size:** about 167 people, three to a sector of 500 (decided 2026-11-07, sized by
  [2026-11-07-community-size.md](../reference/findings/2026-11-07-community-size.md); it replaces
  the unsourced "150 to 500" of mothership-superstructure.md, 2026-07-01).
- **Streets** are paved strips of the ground; homes face them; freight lanes run behind homes and
  never share passenger paths (planning rule, section 2.9 of ship-homes-and-logistics.md).

**Capacity of Drum 1** is the space program's (3.12): about 7,800 people at the default mix (8%
homesteaders), about 11,500 with no homesteads (then the bow's parking spots for private craft are
the limit), about 1,200 if everyone homesteads. The bands above are one way to lay that out; the
program says how much of each band's area the population needs.

The mix is a per-drum setting in data (`data/ship/space_program.toml`, the `share` of each home
kind; ship-homes-and-logistics.md section 2.7). Food land and power set the dense ceiling: the
farms take 122.2 m2 a person fed (the comfortable garden, 3.10), and 10.8 kW a person
(closed-loop-test.md) means one 60 MW reactor pair for each 5,560 people, plus a spare pair.

### 3.6 How a player's home sits in it

- **The plot is the acre** "or whatever it is for that specific ship design" (decided
  2026-09-19). Plot kinds are the home kinds (decided 2026-10-03): Cabin (3.15 x 2.5 m inside a
  block), Apartment (37 to 70 m2 in a block around a common house), Townhouse, House,
  Smallholding, Homestead (55 x 89 m, the default start), Farm. Each kind is a row of
  data/homes/starts.ron with its shipped design; Cabin and Homestead are offered today.
- **One deck for homes, one homestead per plot** (decided 2026-11-06, section 1.1): homes stand
  on the drum floor; homesteads are never stacked one above another; a home may have storeys of
  its own (the homestead's upper floor, 2026-10-20); apartments and other tight-packed homes may
  be multi-storey buildings around an atrium (the block below). Cabin blocks may stack too (the
  space program counts them at two storeys).
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

**The atrium apartment block** (the operator, 2026-11-06: "I wouldn't mind apartments and other
tight packed arrangements being multiple stories. That way we could have like an atrium with trees
and a common gathering area."). A template in data, `data/ship/blocks/apartment_atrium.toml`,
which the space program reads and checks (`node scripts/space-program.js --block`) and rung 3's
generator turns into a design file. Proposal throughout:

```
 ONE RESIDENTIAL STOREY, to scale (1 character = about 2 m)
     +--------------------+
     | U  U  U  CORE U  U |   outer 40 x 40 m; homes 8 m deep, a window on the street side
     | U +------------+ U |   and one on the gallery (dual aspect)
     | U | gallery    | U |   gallery 2 m wide, open to the atrium: every front door
     | U |  ATRIUM    | U |   atrium 20 x 20 m: four trees, the common gathering area,
     | U |  trees     | U |   a glazed roof, the sunline overhead
     | U +------------+ U |
     | U  U  CORE  U  U U |   two cores at opposite corners, a lift and a stair each
     +--------------------+
```

| Part | The template | Why |
|---|---|---|
| Storeys | 4: the ground floor is the common house and the block's services; storeys 2 to 4 are homes; 3.2 m each | the top floor feels 0.96 g on a 250 m drum (computed), within 4% of the street's 1 g |
| Homes | each residential storey: 4 one-person (37 m2), 8 two-person (50 m2), 4 four-person two-bedroom corners (70 m2), the Nationally Described Space Standard's sizes | 48 homes for up to 108 people: the cohousing band of 20 to 50 homes (ship-homes-and-logistics.md section 1.2) |
| Fit | a storey's ring (1,200 m2) minus the gallery (176) and the cores (70) leaves 954 m2; the homes take 828; 126 m2 for walls and services | checked by the script |
| The atrium | 20 x 20 m, planted, with the gathering area (120 m2), seating and four trees; glazed, open to the sunline | every home faces it, so every home has trees in view (3-30-300, 3.13); the sunline reaches the atrium floor from straight overhead |
| Common house | 420 m2 on the ground floor: kitchen, dining room, laundry, workshop, children's room, guest room; with the lobby and parcel room, the plant room, residents' stores and a corner room (cafe, clinic room or shop) | inside the 186 to 743 m2 band of real common houses |
| Lifts and stairs | two cores, each with a lift to every storey and a stair; the farthest door 30 m from a stair along the gallery (Proposal) | every home step-free (WHO, 3.13); two stairs so no single blocked stair cuts a storey off |
| Quiet | bedrooms on the atrium side, away from the street; walls and floors between homes at least 45 dB (England's minimum for new flats) | the noise rules of 3.13 |
| The plot | 50 x 50 m (5 m around the walls for the forecourt, planting and parcel lockers) | 23.1 m2 of drum floor per resident, against about 2,600 for a homesteader (computed) |

### 3.7 Where a new player arrives

```
 THE ARRIVAL PATH (Proposal; the 2026-11-02 brief in order)

 gateway --transport (12+ seats, cargo)--> HANGAR BAY 1 (bow) --walk--> ARRIVALS HALL
                                             cargo unloaded,             the receptionist's desk:
                                             other craft parked          welcome, your bedroom, where
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
- **The noise buffer** between homes and industry (open question 14 of
  ship-homes-and-logistics.md) is now a rule computed from the same source rows, not a fixed
  distance: every bedroom's facade under 40 dB at night from all sources together (3.13). This
  plan still puts every loud source in the spine or at the Aft Landing, so most homes pass by
  distance alone. **The hum has two readers with two falloffs**: the music's audible falloff
  (about 6 dB a room boundary, music.md) so the player hears it, and the health rule's real wall
  attenuation (45 dB for a machine room's walls) for the crew who live with it.

### 3.9 Hangars, vehicle bays, industry, storage, cargo

| Place | Where | Notes |
|---|---|---|
| Hangar Bay 1 and further bays | bow | transports, private craft (decided: "hangar where all the private spaceships are stored"), mission groups gather at a waiting transport (decided 2026-10-03) |
| Mech bays | bow, beside the hangars | work stands with port hookups (mothership-superstructure.md) |
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
| Gardens | each node's garden court; each homestead's own; the greenbelt's orchards. **The garden's goal** (the operator, 2026-11-06): "Our goal is a minimalist survival garden that provides all the nutrients it possibly can." Designed in [survival-garden.md](survival-garden.md); every new homesteader starts with its comfortable garden (decided 2026-11-07: "The comfortable one."), which fits the homestead's greenhouse (the space program checks it) |
| The ship's farms | each sector's farm band (3.15); 122.2 m2 of growing floor for each person they feed, the comfortable garden's own figure (3.12), on one level (decided 2026-11-07) |
| The Arena and the Armory | greenbelt (the field) and the bow (target ranges): sport by consent (decided 2026-11-03) |

### 3.11 Transit

| Mode | Route | Numbers |
|---|---|---|
| Walking | streets on the ground | 1.4 m/s design; the game walks at 5 m/s |
| Axial rail on the drum floor | Fore Landing, greenbelt, Aft Landing | 2 km in 3.4 to 4.8 minutes at 25 to 35 km/h (section 2.6 of ship-homes-and-logistics.md) |
| Spoke lifts | 4 down each end cap | 100 s rim to axis at 2.5 m/s; every lift step-free, with a stair beside it (3.13) |
| Spine passenger rail | along the spine, bow to stern | between sectors |
| Spine freight rail | along the spine | goods only (planning rule) |
| Small craft | bow hangars | between motherships and out to nearby bodies (they jump, decided 2026-11-03; since 2026-11-07 the motherships cross between stars by a jump too, 3.14) |

The resilience rule of docs/game/ship_zoning_transit.md:77-110 stands: every critical place has a
fast route and a slow one, and no single lift or line failure isolates a district (the stairs in
each end cap are the slow route).

### 3.12 The space program: how the ship's size is measured

His question (1.1): "How do we end up measuring the total size of the ship? Do we need to plot the
size for each home, hangar, engineering bay, industrial zone, reactor room, bridge, storage areas,
transportation like railways and elevetors, etc?"

**The answer: each KIND of space, once, as a row; never each instance.** This is how architects
size a building (a "space program": the list of every kind of room with its area) and how NASA's
1975 summer study sized a 10,000-person settlement (NASA SP-413, Table 3-2: every space use with
its area per person, its storeys and its height). A row gives an area per person, or an area per
unit and how many people one unit serves; the population multiplies; circulation and structure
are added where a figure is a bare room; the rows add up by zone (the drum floor, the spine's
decks, the bow's decks); and the room Drum 1 offers is the check. The layout then places the
instances (sector and neighbourhood templates, rung 3), and the planning tests check the layout
against the same rows. Change a number and the ship changes.

- **The program**: [data/ship/space_program.toml](../../data/ship/space_program.toml), each row
  naming its source. **The sources**, quoted and dated:
  [2026-11-06-space-program-sources.md](../reference/findings/2026-11-06-space-program-sources.md),
  [2026-11-07-community-size.md](../reference/findings/2026-11-07-community-size.md) (the sector),
  [2026-11-07-drum-limits.md](../reference/findings/2026-11-07-drum-limits.md) (the scale study).
- **The tool**: `node scripts/space-program.js 1000` (the totals for 1,000 people and whether
  Drum 1 holds them), `--mix homestead=0.05,cabin=0.25,apartment=0.70`, `--farm-levels 2`,
  `--rows` (every row), `--sector` (the 500-person sector, 3.15), `--scale` (1 and 10 billion,
  3.16), `--block` (the apartment block), `--sources`. Its tests are in `just rig-tests`.

| Group | The rows (m2 a person unless noted) | Zone | Source |
|---|---|---|---|
| Homes | a homestead plot of 4,895 m2 plus 31.5% for its street and gap, for 2.5 people; a cabin of 7.5 m2 plus 2 m2 of shared rooms, with circulation and structure, on 2 storeys; an apartment from the block template (23.1 m2 of floor a resident) | drum | the home kinds (ship-homes-and-logistics.md 1.2), the block template |
| Community | shops 2.3 (2 storeys), offices 1 (3), schools 1 (3), hospital 0.6 (2 storeys, at least 300 m2; modern area a bed, 2026-11-13), assembly halls 1.5, recreation 1, public open space 10 (at least 0.5 ha a neighbourhood), workshops 4 (2), streets and paths 12, the rest 2.9 (3) | drum | NASA SP-413, Table 3-2 |
| Neighbourhoods and landings | a commons for each 167 people (1,870 m2, today's Commons; three to a sector), General Services for each 2,500, 3 rail stations and 8 spoke lifts a drum | drum, end caps | the ship file, the community-size finding, Proposal |
| The ship's farms, for everyone whose home does not feed them | growing floor 122.2 (the comfortable garden with its paths) on one level under the sunline, `[farms] levels` allowing two or more; animals 5, processing 4, drying 8, on 3 levels | drum | survival-garden.md (comfort tier), SP-413 |
| Life support and power | water and waste 4, electrical 0.1, communications 0.05, the air plant 0.3 (at least 500 m2), a reactor pair (4,000 m2) for each 5,560 people plus a spare pair, a control room a drum | spine | SP-413, closed-loop-test.md, Proposal |
| Storage, industry, freight | storage 5, heavy industry 6, freight rail and tunnels 1 | spine | SP-413, Proposal |
| The bow | the bridge 800 m2 and the Arrivals hall 1,500 m2 once; a bay for each transport class, sized from a real craft and a real clearance: shuttle (S-92 class) 1,140 m2 for each 2,000 people, ferry (ATR 72 class) 1,520 m2 for each 5,000, freighter (C-130 class) 3,290 m2 and heavy lift (one 100-tonne-plus vehicle) 5,950 m2 of deck for each 10,000, at least one of each; a parking spot for a private craft (200 m2, a four-seat R44's size with its clearance) for each 20, a mech bay (600 m2) for each 2,000, a drone bay (300 m2) for each 500 | bow | [the sizes research](../reference/research/2026-11-13-mothership-sizes/README.md) section 2; the counts Proposal |
| Outside the hull | radiator fin 67 | exterior | closed-loop-test.md |
| Allowances | circulation x 1.4 and structure + 10% on rooms given bare | | GSA 2012 (1.4 to 1.6), Proposal |

**The room Drum 1 offers** (computed): the drum floor, 2 pi x 250 m x 2,000 m = 3.14 km2; the
spine segment's 340 decks of 2,827 m2 at 80% usable = 0.769 km2; the bow's 20 decks of 11,310 m2
at 70% usable = 0.158 km2.

**Results at the default mix** (8% of residents homesteaders, 22% in cabins, 70% in apartment
blocks; Proposal), from `node scripts/space-program.js` (2026-11-07, with the ship's farms growing
the comfortable garden; until then they used NASA's 65.29 m2 and Drum 1 held 9,437):

| People | Drum floor | Spine decks | Bow decks | Floor a person | Drum 1 |
|---|---|---|---|---|---|
| 100 | 0.053 km2 (2%) | 0.011 km2 (1%) | 0.017 km2 (10%) | 798 m2 | holds them |
| 500 (one sector) | 0.206 km2 (7%) | 0.017 km2 (2%) | 0.021 km2 (13%) | 488 m2 | holds them |
| 1,000 | 0.407 km2 (13%) | 0.025 km2 (3%) | 0.026 km2 (16%) | 458 m2 | holds them |
| 10,000 | 4.03 km2 (128%) | 0.179 km2 (23%) | 0.130 km2 (82%) | 434 m2 | **does not**: two drums |

(The bow column changed on 2026-11-13, when the bays were sized from defined transport classes and
the parking spot from a real craft; the drum floor did not move.)

The most Drum 1 holds: **7,781** at the default mix (about 15 sectors); 9,031 with the farms on two
levels; 11,500 with no homesteads (then the bow is full, of parking spots for private craft); 1,187
if everyone homesteads. At 10,000 people it holds them with at most 4% homesteaders.

What the numbers say (computed from the program):

- **Homesteads are the ship's hungriest use of floor**: about 2,600 m2 of drum per homesteader (the
  plot, its street and its own food) against about 130 to 150 m2 for a cabin or apartment
  resident's home plus their share of the farms (both before the streets, parks and civic places
  everyone shares, about 36 m2 a person). At 7,781 people the 8% who homestead take 51% of the drum.
- **The farms come next**: 1.18 km2 for the 9,200 people they feed at 10,000. A second level under
  lamps frees 0.56 km2 (the lever when the floor runs short, at a cost in power; the operator's
  "allow for two or more").
- **Small populations pay for minimums**: a hospital, a hangar bay and a bridge for 100 people
  make 798 m2 a person; at scale it settles near 435.
- **The spine has room to spare** (23% at 10,000): industry and storage can grow.
- **The bow is the limit of a dense drum**, because of one private craft's parking spot for every
  20 people (question 15).
- **Against NASA's settlement**: SP-413 planned 67 m2 of projected area a person at about 15,000
  people a km2; Drum 1 at the default mix gives about 400 m2 of drum a person, about 2,500 people a
  km2, because homesteads, one-level sunlit farms and a garden that meets every nutrient are
  generous choices.

### 3.13 Crew health: the rules a layout is checked against

The operator, 2026-11-06: "We want to maximize crewmember health." So every rule below takes the
strictest health-based value where sources differ (the finding's judgement, labelled there). The
sources are quoted and dated in
[2026-11-06-noise-and-sleep.md](../reference/findings/2026-11-06-noise-and-sleep.md) (F numbers
below). Each rule is a check over the layout's data; the last column says which rung builds it.

| Rule | The value | Source | Checked in |
|---|---|---|---|
| Night noise outside every home | at most **40 dB Lnight** at the facade, all ship sources together, over the sunline's 8-hour night; never the 55 dB interim target | F4 | rung 5, the noise rule over the source rows |
| Inside bedrooms at night | at most **30 dB LAeq** over the night; no event above **45 dB LAmax**, and none more than 10 dB above the background | F3, F5 | rung 5 |
| Low-frequency sound (the hum, fans, pumps) | 5 dB below the bedroom values (Proposal: the WHO says "a still lower guideline value" without a number) | F3 | rung 5 |
| Inside homes by day | at most 35 dB LAeq over 16 hours | F3 | rung 5 |
| Streets, parks, gardens by day | at most 50 dB LAeq over 16 hours | F3 | rung 5 |
| Between homes | walls and floors at least 45 dB airborne insulation (DnT,w + Ctr), floors at most 62 dB impact | F7 | the home and block designs (the template carries it) |
| A home's shell | counted as 25 dB (a closed window); a better wall is a design choice, never assumed | F2 | constant in data |
| Workplaces in the spine | continuous noise within NC-50; never above 85 dBA | F5 | rung 5 |
| Daylight | at least **250 lux melanopic EDI at the eye** in homes, schools and workplaces during the sunline's day | F8 | rung 2, the sunline and windows |
| Evening | at most **10 lux** melanopic EDI from 3 hours before bedtime: the sunline dims, street lights are shielded and warm | F8 | rung 2 |
| Night | at most **1 lux** in sleeping rooms | F8 | rung 2 |
| One shared day | a day and a night in every home (NASA's 1975 criterion); farms may run their own light in enclosed fields | F9 | rung 2 |
| Green space | public green space of at least 0.5 ha, 1 ha preferred, within **300 m** in a straight line of every home | F10, F11 | rung 3, the planning tests |
| Trees | at least 3 trees in view from every home; at least 30% tree canopy in every neighbourhood | F11 | rung 3 |
| Open space in all | at least 10 m2 a person | F12 | the space program (built) |
| Walking | a mess hall within 5 minutes (a test since increment 6); every daily need within 15 minutes | ship-homes-and-logistics.md 2.9 | rung 3 |
| Air | homes never share an air zone with industry or the reactors (the bearing's seals separate the drum's air from the spine's); every home's zone within the WHO's 2021 levels (PM2.5 5 µg/m3 a year, 15 a day; NO2 10 a year, 25 a day; and the rest) and carbon dioxide below 0.4 kPa | F15, F16 | rung 5, the zones and the life support model |
| Crowding | no household above its home's design occupancy; occupancy set by bedrooms (at most two a bedroom, by the bedroom standards the WHO lists) | F13 | the block template (built); NPC households in the crowd tiers (rung 9) |
| Access | every home step-free; a lift to every storey of every multi-storey building, with a stair beside it | F14 | the block template (built) |
| Warmth | homes kept warm enough (the WHO's 18 °C for cold seasons as the floor) | F14 | ship-life-support.md's model |

Not yet a rule: **how fast the drum may spin**. NASA's 1975 study set "≤1 rpm" (its Table 3-3);
Drum 1 spins at 1.89 rpm (its size decided on 2026-09-15; habitat-generation.md notes the spin is under "the ~2 rpm Coriolis comfort limit"); the bulk drums of 3.16 spin at 0.95 rpm, inside NASA's figure.
The later research on adapting to rotation has not been read; it is the next finding for crew
health, and the size stays decided meanwhile.

**How far is far enough** (Proposal, the standard free-field estimate over a hard floor): a source
of sound power Lw, at distance r metres, through boundaries that each take off A dB, gives
Lp = Lw - 20 log10(r) - 8 - (sum of A) at a facade. For the 40 dB night target:

| The source | Distance it needs (computed) |
|---|---|
| A 100 dB(A) machine standing in the open | 398 m |
| A 90 dB(A) machine standing in the open | 126 m |
| The same 100 dB(A) machine in a room with 45 dB walls | 2.2 m |

So enclosure beats distance: the rule is computed per bedroom from the source rows, every loud
machine stands in a room whose walls take off at least 45 dB, and the loudest stay in the spine
behind the bearing. Sound carried by the ship's frame itself (the operator's 2026-07-01 "the noise
comes through the steel frame") has no sourced figure yet (the finding's open questions): every
machine row carries a vibration mount, and the bearing between drum and spine is the break.

**The hum's two readers**: the brown-noise ship hum (music.md) is an audio effect the player hears
at the player's own volume, and a low-frequency sound the simulated crew lives with. Only the
second counts against these rules, at the low-frequency values.

### 3.14 The cruise is a jump; thrust only to manoeuvre; how the floors take the lean

His question, 2026-11-06: "So this means while on the ship we'd be in 5% earth gravity?" **No**
(answered then: the spin gives 1 g on the drum floor, and thrust only leans it). His answer,
2026-11-07: "We can introduce some sort of mechanic that requires no thrusting. Like a teleportation
tech requires no thrusting." **Decided: the motherships cross between the stars by a jump, with no
thrust while travelling.**

**Why a jump, in real physics.** Steady motion gives no weight at all (Newton's first law); only a
change of speed does. Burning all the time at 0.05 g reaches 5.2% of light speed in a year and 47%
in ten, and any drive that throws propellant must then start about 5.6 times as heavy as it ends
for one year and about 30 million times for ten (with an assumed exhaust speed of 3% of light, the
rocket equation; computed in this plan's 2026-11-06 pass). Burn, coast, burn takes about 88 years
to Alpha Centauri. So a fleet that moves between stars within a lifetime needs a fiction either
way, and the jump is the one the game already has for small craft.

**The jump, as the game plays it (a game fiction, said so in the game's words, like the link in
game-setting-and-intro.md 2.5):**

- The jump drive is charged from the reactors over hours (the charge time, the reach and the energy
  are data rows, Proposal: `data/fleet/drive.ron`, beside the fleet's upgrade lines of
  game-setting-and-intro.md 3.4, where "Cruise speed" becomes the motherships' jump reach).
- During the jump nothing pushes: the drum floor is a plain 1 g straight down and the spine is
  weightless. The ship arrives "moving with whatever it jumped to" (game-setting-and-intro.md 3.1),
  so no burn is needed to stop.
- **Real thrust is for manoeuvring only**: docking, station-keeping beside a body, turning, small
  course changes. Thrusters at the bow pull the ship along its axis (3.16: a long spine is pulled,
  never pushed), and small attitude thrusters turn it.
- **This changes two earlier decisions**, which the plan records rather than hides: the 2026-11-03
  "small craft jump, the motherships never do" (game-setting-and-intro.md 10.1; question 10b asks
  him to confirm the change), and the premise of the 2026-09-15 decision that the spine has
  gravity, since a jumping ship's spine is weightless (3.3).

**What you feel during a manoeuvre.** The push adds along the axis, at right angles to the spin's
pull, so "down" leans toward the stern by arctan(a/g) and the total is a hair above 1 g:

| Manoeuvre | Lean | The floor, compared with floors people walk | A 10 m pool laid along the axis | In the spine (a 70 kg person) |
|---|---|---|---|---|
| Routine, 0.02 g | 1.15 degrees (1 in 50) | flatter than the 1 in 48 cross slope the ADA allows on any walkway | 0.2 m deeper at its stern end | weighs 1.4 kg |
| The cap, 0.05 g | 2.86 degrees (1 in 20) | exactly the steepest walking surface the ADA does not call a ramp; a ramp may be 1 in 12 | 0.5 m deeper | weighs 3.5 kg |

(The ADA's 2010 Standards, sections 403.3 and 405.2, quoted in
[2026-11-07-drum-limits.md](../reference/findings/2026-11-07-drum-limits.md), F8.) An emergency that
would need more than 0.05 g is a jump instead, so no burn ever exceeds the cap.

**His drum question**: "I'm wondering how we'd design a drum design that has subsections that are
able to adjust to either one or two axis. Gyroscopes start add a lot of excess structure."

First, **one axis is always enough.** The lean is along the ship's axis whenever the push is along
the axis. A sideways push on a spinning drum would turn with the spin, so in the drum's own frame
the floor would rock back and forth once a turn (every 31.7 s at 1.89 rpm): no slow mechanism can
follow that, and it would feel like a boat in a swell. So the ship never pushes sideways while the
drums spin; it turns first (slowly, by attitude thrusters), then pushes along its axis. Every
option below then needs at most a tilt toward the stern and back.

| Option | Structure it adds | Mass | How it fails | What a player feels |
|---|---|---|---|---|
| **A. Fixed floors and a thrust cap** (do nothing to the floors; cap the push at 0.02 g routine, 0.05 g at most, axial only, ramped over about half a minute) | none | none | nothing structural; a burn above the cap is refused by the drive's controller | during a manoeuvre a gentle slope toward the stern: a ball or a wheeled cart rolls aft, water in pools and tanks tilts, a chime and an on-screen "manoeuvring" mark; at 0.02 g flatter than the cross slope any walkway may have |
| **B. Pendulum decks** (deck segments hung on trunnion pins along a circumferential axis, swinging passively like a ship's gimballed stove, with dampers) | pins, bearings and dampers under every segment; a gap at every segment joint; flexible links for every pipe, cable and duct crossing a joint; buildings built as hung boxes | the hangers carry the whole deck (homes, soil, water) in bending to the pins: a second floor structure under the first | a pendulum on 3 m arms swings with a 3.5 s period (2 pi root(l/g), computed) at the start and end of each burn unless damped; a failed damper keeps it swinging (sickness); a seized pin makes it a fixed floor again; a joint that steps is a trip hazard | the floor stays level, but the joints shift: a 20 m segment tilted 2.86 degrees moves its ends 0.5 m, so every door and street crossing a joint steps during a burn, and the neighbourhood sways briefly |
| **C. Floors on jacks** (floor plates on actuators that tilt a few degrees) | actuators, sensors, control and power under every plate | moderate | a stuck jack leaves a floor tilted after the burn; a control fault tilts the wrong way; power loss mid-burn | the floor moves under you; only small plates can tilt (a 55 m homestead tilted 2.86 degrees rises 2.75 m at one end, breaking every threshold), so in practice it is the bed, the bath, the pool's floor |
| **D. Tilt the whole drum's frame** (gimbal the drum against the thrust) | the gimbal rings around a 500 m drum: the "excess structure" he named | enormous | tilting the spin axis puts part of the push across it, and that part turns with the spin: the lean would wobble once a turn instead of holding still, which is worse | a slow rocking every 31.7 s |
| **E. Items, not floors** (the sailor's answer: gimbal only what must stay level; baffle what sloshes) | baffles in pools, fish tanks and water stores; lips on shelves; a gimballed stove top where a pot boils | small | none that matters | pools and tanks barely move; nothing falls off a shelf |

**Recommended: A with E**, because the lean is then never steeper than floors people walk every
day without noticing (the ADA's cross slope at the routine push, its walking-surface limit at the
cap), it adds no structure and no failure, and it keeps the physics honest and visible (balls roll,
water tilts) at a level that teaches rather than annoys. B and C solve a problem the cap removes,
at the cost of joints that step and machinery that can jam; D makes it worse. **Simplified mode**
keeps every floor visually level during a manoeuvre and gives everyone in the spine magnetic
boots that are always on. The drive's state (jumping, charging, manoeuvring at a push) is data,
and the lean and the spine's weight follow it.

### 3.15 The sector: a community of 500

His words, 2026-11-07: "Let's start with a sector for 500 people. I've heard that communities of
500 people is about the upper limit for one massive group to work together. Can you research that
number?"

**What the research says** ([2026-11-07-community-size.md](../reference/findings/2026-11-07-community-size.md)):
500 is real, but it is the outer ring, not the working group. In Dunbar's layers about 500 is the
circle of acquaintances, people known well enough to talk with (F5); the group held together by
personal relationships is about 150, and communities that govern themselves without written law
split between about 150 and 200 (Hutterite colonies at a mean of 166, Alpine commons at 140 to 176
for six centuries; F12, F13, F15). 500 is also where the record shows communities starting to need
a formal structure: officials, roles, shared rules (kibbutzim settle near 468, campsites cluster
near 500; F12, F16). Every one of these is an average with a wide spread, and the brain-size
regression behind 150 is disputed (intervals of 4 to 520 in the 2021 critique, F8; Dunbar's reply,
F9). **So his 500 is right as the largest community in which everyone can still know everyone by
sight, and the sector keeps it as its cap; the group that works together is built in at about
150.**

**Sector 1's role (decided here, Proposal):** Sector 1 is now **one 500-person sector**, the first,
at the fore end of Drum 1, and the drum he sized is **Drum 1**, holding about 15 by area (12 as
laid out in 3.4). Why: his word "sector" now names the 500-person community, and one concept keeps
one word (CLAUDE.md, account words; the same rule for names); his 250 m x 2 km drum is kept whole
under its own name; rung 1 still builds in Sector 1, so nothing already planned moves; and at a
billion people the sector is the unit everything repeats (2 million of them), which only works if
the word means one thing.

**The layers, built into the space** (the finding's judgement table, at a ratio of about three):

| Layer | People | Built as, aboard | Shared places |
|---|---|---|---|
| Home | about 5 (1 to 6) | one home: a homestead (2.5 at the default), an apartment, a cabin | its own rooms |
| Cluster | about 15 | a cabin hall of 16 cabins; six homesteads on one lane; one corner of an atrium storey | a small common room or court |
| Block | about 50 | one storey of the atrium block (16 homes, 36 people); a homestead street (16 homesteads, 40) | a garden court, laundry, play space |
| Neighbourhood | about 167, split rather than grown past about 200 | the node (3.5) and the homes within 5 minutes of it | the mess hall, the commons garden, a room where every adult fits for an assembly |
| **Sector** | **500** | three neighbourhoods and a civic heart | the civic hall (the assembly rows give each sector 750 m2), the school, the clinic, the workshops, the market row, the sector's council |
| District | about 1,500 | three sectors | General Services, a rail station, the greenbelt's arena and lake |

**What one sector holds** (`node scripts/space-program.js --sector`, the default mix): 16
homesteads (40 people), 110 cabins (110 people, seven halls of 16), 4 atrium blocks (350 people,
432 places), 3 commons, the civic rows, and the farms for the 460 people the homes do not feed
(56,212 m2 of growing floor on one level, the comfortable garden; plus animals, processing and
drying). **Drum floor 0.206 km2** for a sector on its own (413 m2 a resident, minimums such as a
hospital and a hangar bay included); 0.202 km2 once a full drum shares them. Spine decks 0.017
km2, bow decks 0.021 km2 (one bay of every transport class). It draws 5.4 MW at the closed-loop test's 10.8 kW a person.

**Its drum size and its walks.** On a drum of Drum 1's radius (250 m, 1.89 rpm):

| Cut | Around x along | Farthest home to its mess hall | Farthest home to the civic hall | Walking rules (5 and 15 minutes) |
|---|---|---|---|---|
| A whole ring | 1,571 x 131 m | 328 m, 3.9 min | 851 m, 10.1 min | met |
| **A half ring** | **785 x 263 m** | **262 m, 3.1 min** | **524 m, 6.2 min** | **met** |
| A quarter ring | 393 x 526 m | 284 m, 3.4 min | 459 m, 5.5 min | met |

(Streets, around plus along, at 1.4 m/s; the three neighbourhoods side by side along the longer
side, each mess hall at its middle, the civic hall at the sector's middle.) **Recommended: the half
ring.** It has the shortest walk to the mess halls, and two half-ring sectors fill a ring 263 m
long, 1.05 radii: inside the 1.3-radius spin-stability limit (3.16), so one ring of two sectors
could be a drum of its own, stable even flying free. The quarter ring walks a minute less to the
civic hall but is 526 m long, past the limit for a drum of its own.

```
 ONE SECTOR, a half ring of Drum 1, unrolled (to scale: 1 character = about 20 m around, 22 m along)
 s (around) ->  0                   262                  524                  785
 a (along)   0 +--------------------+--------------------+--------------------+
               | N1  about 167      | N2  about 167      | N3  about 167      |
               |  homes, its green  |  homes, its green  |  homes, its green  |
               |      [node]        |  [node] [CIVIC]    |      [node]        |
               |                    |  school, clinic,   |                    |
               |                    |  market, hall      |                    |
           188 +--------------------+--------------------+--------------------+
               | workshops in 45 dB rooms | THE FARM BAND: the comfortable garden for |
           263 | the 460 the homes do not feed, on one level; animals, drying     |
               +-----------------------------------------------------------------+
```

- **Homesteads are half the sector's floor** (16 plots, 0.103 km2). A neighbourhood is equal to the
  others in people, not in area: one that holds all 16 homesteads and the cabins is about 430 m
  around (its farthest home 4.1 minutes from its mess hall, 8.8 from the civic hall, computed),
  and the other two, of atrium blocks, are compact. The layout generator (rung 3) places them.
- **The quiet-bedroom rule (3.13)**: the workshops, the loudest place on the floor (a 90 dB(A)
  machine, Proposal), stand in rooms whose walls take off 45 dB, which keeps a bedroom's facade
  under 40 dB at night from 0.7 m; in the open they would need 126 m (computed, `--sector`). So
  they sit in their rooms on the sector's aft edge beside the farms, a street from the nearest
  home, and the atrium blocks keep their bedrooms on the atrium side (3.6).
- **Rung 1** (6.4) builds N1 of Sector 1 flat, with its 12 homestead plots (most of the sector's 16
  at the default mix), the 16-cabin hall and its commons, in the 393 m x 300 m cell it already
  has; rung 3 re-cuts the whole sector from its template.
- **Data**: `[sector]` and its layers in data/ship/space_program.toml (built); a sector template,
  `data/ship/sectors/half_ring.toml`, for rung 3's generator (Proposal).

### 3.16 One billion and ten billion: a fleet

His words, 2026-11-07: "Let's aim for 1 billion and 10 billion."

**Sectors**: 2,000,000 for a billion and 20,000,000 for ten billion. Everything below is real
physics, computed by `node scripts/space-program.js --scale` from data/ship/space_program.toml,
the limits sourced in [2026-11-07-drum-limits.md](../reference/findings/2026-11-07-drum-limits.md);
the jump that moves the ships is the one game fiction, and it plays no part in the sizes.

| Limit | What decides it | The number |
|---|---|---|
| How wide a 1 g drum can be | hoop stress: the rim carries its own weight as density x g x radius (F1), plus air and contents; working strength = strength / 2 | steel at most 4.5 km even empty; aramid composite 62.8 km; carbon composite 83.3 km. A carbon rim is 2.2 cm thick at 250 m (34 kg/m2), 8.8 cm at 1 km (138 kg/m2) |
| How long a drum can be | spin stability: a free cylinder spins stably only up to 1.3 radii long (F2) | 325 m at 250 m radius (Drum 1 is 2,000 m: question 1b); 1,300 m at 1 km |
| How long a ship can be | the spine carries every drum ahead of or behind the thrusters during a 0.05 g manoeuvre; held to a tenth of the ship's mass, a carbon composite spine may be 333 km | ships of 332 km |
| How a spine is pushed | Euler buckling: pushed from the stern, a spine sized for that load buckles past about 1.4 km between braces (computed) | the manoeuvring thrusters pull from the bow, or each drum carries its own |
| Mass | the shield (10 t/m2 of water for 5 mSv a year, decided in 1.5; the sizes research, section 7, corrected the old 4.5 t/m2, which gives about 55) dominates, held in a non-rotating shell around the drums so it carries no spin and loads no drum (F1's "unattached" design) | about 5,220 t a person in 1 km drums (2,650 at the old 4.5 t/m2) |
| Where it comes from | the main asteroid belt holds about 2.4 x 10^21 kg (F7) | a billion people's fleet is about two millionths of it; ten billion's about two hundred-thousandths |

**See also: [the active-shielding research](../reference/research/2026-11-13-active-shielding/README.md)**
(2026-11-13), on whether anything beats the water shield. Drum 1's steel is 17.5 cm and the shell
10 m of water (about 11.5 m in foam cells); magnets cut the cosmic-ray dose by tens of percent, not
the factor of about 100 needed, and add less the thicker the water, so they are not proposed for
the drums (a [dated negative finding](../reference/findings/2026-11-13-active-shielding.md)); the
ship's own floor, soil and stores are probably worth 2 to 3 t/m2 of the water, to be confirmed by a
transport calculation before the 10 t/m2 here changes.

**Which drums.** Drum 1 keeps its decided 250 m radius. For the billions the study compares two
drum sizes, each as long as the stability limit allows:

| Drum | Spin | Length | Floor | Sectors | People | Mass of one drum |
|---|---|---|---|---|---|---|
| Drum 1's radius, 250 m | 1.89 rpm | 325 m | 0.511 km2 | 2 | 1,000 | 6.4 x 10^6 t |
| **The bulk drum, 1 km** | **0.95 rpm** | **1,300 m** | **8.17 km2** | **40** | **20,000** | **9.5 x 10^7 t** |

The 1 km drum is recommended for the bulk of the population: it spins under NASA's 1975 "1 rpm"
(3.13), holds four times the people per kilometre of spine, and its carbon rim is still thin. Its
40 sectors sit as patches around and along the floor (13 districts of three).

**One ship cannot hold them.** In 1 km drums a billion people make a spine 67,500 km long (0.23
light-seconds end to end); carrying its own manoeuvring loads it would weigh about 20 times what it
carries, by an estimate that leaves out its own weight, which only makes it worse. Ten billion:
675,000 km, past the Moon's distance, and 200 times. humanity_one.md's single 500 km hull fails the
same two tests (it is past the spine's length and many radii long).

**The honest answer is a fleet:**

| | One ship | A billion | Ten billion |
|---|---|---|---|
| Ships | 1 | **about 204** | **about 2,033** |
| Drums (1 km) | 246 | 50,000 | 500,000 |
| Sectors | 9,840 | 2,000,000 | 20,000,000 |
| People | 4.92 million | 1 billion | 10 billion |
| Length | 332 km | 204 ships of 332 km | 2,033 ships of 332 km |
| Mass | 2.6 x 10^10 t | 5.22 x 10^12 t | 5.22 x 10^13 t |
| As one rock (2,000 kg/m3) | about 2.9 km across | 17.1 km across | 36.8 km across |

(In drums of Drum 1's radius it would take about 1,127 and 11,262 ships of 888,000 people each.)
The shield sets the mass, not the ship count: the spine's longest length does not depend on what
it carries, so moving the shield from 4.5 to 10 t/m2 (1.5) about doubled every mass above and
changed no count of ships, drums or sectors.
**Every new sector is about 2.6 million tonnes, one rock about 135 m across**, most of it the
shield's water, the bulk the fleet's mining (drone-mining.md, at industrial scale, not the
personal drone) brings in. Growing the fleet is a fleet project, a row in the fleet ledger.

**The ship's drums turn in pairs** (Proposal): two drums spinning opposite ways cancel each other's
spin, so the ship turns without fighting the gyroscopic push of 246 spinning drums; the spine
carries each pair's equal and opposite push between them. People cross between drums through the
end caps and the spine rail, as they already cross between Drum 1 and any later drum.

**What the game builds and what streams** (CLAUDE.md, infinite-of-x and the compute budget). The
game never instantiates the fleet. A sector is a template plus a seed, so 20 million sectors are
generated on demand from (ship, drum, sector) and stored only when someone touches one (a claimed
plot, an edit). The tiers of section 5 grow two more:

| Tier | What | Cost (Proposal) |
|---|---|---|
| L0 to L2 | your room, your neighbourhood, your sector and drum's floor (section 5) | as section 5 |
| L3 | your ship's spine, end caps and other drums: one silhouette per drum class, instanced 246 times | 0.5 ms |
| L4 | the fleet: other ships as distant impostors, drawn only when in view | 0.1 ms |
| Population | every sector beyond yours is a counter in crowd-simulation.md's aggregate tier, updated a slice per frame, obeying the Dev page's "turn off all reuse" switch | under 0.1 ms CPU |
| Network | a server sends only your sector's state and the fleet's ship positions and jump states, at a low rate; a server holds only the sectors players have touched | |

## 4. From one player to an MMO server (infinite-of-x)

Everything above is rows, not code (Proposal for the shapes):

| Data | What a row is | Example |
|---|---|---|
| `data/ship/drums.ron` | a drum: its class (radius, length, spin), its band rows, its spine segments, its sector instances | `(id: "d1", radius_m: 250.0, length_m: 2000.0, ...)` |
| `data/ship/bands/*.ron` | a band of a drum floor: from, to, use | `(from_m: 120.0, to_m: 383.0, use: "sectors")` |
| `data/ship/sectors/*.toml` | a sector TEMPLATE (3.15): its cut (half ring), its three neighbourhoods, its civic heart, its farm band, its workshops | `half_ring` |
| `data/ship/neighbourhoods/*.ron` | a TEMPLATE: the node's design, street pattern, plot slots by kind, size | `n-mixed-a` (12 homesteads, 16 cabins, a commons) |
| `data/fleet/ships.ron`, `data/fleet/drive.ron` | a ship of the fleet (its drums by class and seed, its position, its jump state); the jump drive's reach, charge time and the manoeuvring cap (3.14) | 204 ships for a billion (3.16) |
| ship file (data/blueprints/ship_structure.ron) | the authored or generated instances: neighbourhoods placed by (drum, sector, s, a), the ground layer, zones, plots with stable ids | plot `d1-s1-n1-h03` |
| `data/blueprints/zone_types.ron` | each zone type's colour, its default hum source and `thermal_output` | |
| `data/audio/hum_sources.ron` (music.md) | a hum source: kind, position in ship metres, strength, colour | the sector's reactor pair |
| `data/homes/starts.ron`, `data/homes/shipped/*.ron` | home kinds and their designs | existing |
| `data/ship/space_program.toml` (built 2026-11-06, the sector and the scale study 2026-11-07) | a kind of space: its zone, area per person or per unit, storeys, source; the home mix; the sector of 500 and its layers; the drum it is checked against; the materials and drum classes of the scale study | `streets`: 12 m2 a person, SP-413 |
| `data/ship/blocks/*.toml` (built 2026-11-06) | a building TEMPLATE: storeys, unit kinds and counts, atrium, cores, common rooms, the rules a design from it keeps | `apartment_atrium` |
| `data/asteroids/*.ron`, `data/mining/drones.ron` ([drone-mining.md](drone-mining.md) section 9) | a rock with its frame, distance and ores; a drone class with its propellant and hold | the drone mining rungs |

- **Offline, one player**: N1 is built in full; your home stands on the default plot; every other
  plot is drawn as its kind's shipped design (built since increment 2). The rest of Sector 1 and of
  Drum 1 is drawn from the same templates at lower detail (section 5).
- **A server**: the relay hands out plots in a claim order (N1's first, then N2's, then the next
  sector's), by stable id (`game_plots`, built in increment 1b). A full kind gets a waiting list
  (open question 4 of ship-homes-and-logistics.md). Players meet anywhere; the relay delivers what
  is within 250 m and drops it past 300 m (data/ship/shared_world.ron), about one neighbourhood.
- **An MMO**: a neighbourhood is the unit of interest, a sector of 500 the unit of sharding (3.15);
  a full sector opens the next, like a Hutterite colony splitting; the population beyond the
  players is crowd-simulation.md's three tiers (an aggregate per sector, a roster for the loaded
  sector, an embodied few near you), so the cost scales with what you can see, not with who lives
  aboard. At a billion and ten billion the fleet streams as 3.16 says.
- **Generated or authored**: rung 3 adds a generator that turns a template plus a placement into
  plots with stable ids; any neighbourhood may instead be authored by hand in the editor (the
  construction tools serve both, decided 2026-09-19). The planning rules (5 minutes to a mess
  hall, 15 minutes to every daily need, no home within the noise buffer, freight apart from
  passengers, no single failure isolating a district) run as tests over whatever the data says,
  and so do the crew-health rules of 3.13 and the space program's totals (3.12).

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
| L3 | the spine, the end caps, other drums, the exterior | silhouettes from data, one per drum class, instanced (3.16) | 0.5 ms |
| L4 | the other ships of the fleet | distant impostors, only when in view (3.16) | 0.1 ms |
| | sky, post, UI | | 2.2 ms |

- **Memory** (Proposal, to measure): L2 at about 5,000 triangles a neighbourhood of 167 (a third
  of the old 300-person cell's 15,000), about 45 in Drum 1 (15 sectors of three), about 225,000
  triangles; one homestead design's shell is shared by every instance of it. The
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
| The whole layout of the ship file: zones `commons`, `street-1`, `cabins-1`, `commons-2`; 3 corridors; 28 plots; 14 districts; `id: "mothership-1"` | data/blueprints/ship_structure.ron:100-1661 | Rebuilt for Sector 1 (rung 1); the new `id` starts every plot afresh on a relay, which is intended (no compatibility before v1.0). The districts' removal is decided (2026-11-06: "Sounds good.") |
| The Commons' 15-wall room block (the superseded 12-room house) | ship_structure.ron:327-720 | Audit finding 5; retiring it is decided (2026-11-06: "Let's try it.") |
| The Commons' six lights outside its box | ship_structure.ron:757, 768, 779, 790, 812, 823 | Audit finding 4 |
| The Commons' machine positions (garden, stalls, mess, recreation) | data/machines/ship.ron | Re-placed in N1's commons; ids kept so the garden's crops carry |
| All nine wild-spawn rows | data/entities/wild_spawns.ron:24-49 | Planet-era absolute positions (audit section 5); decided 2026-11-06: no wild nodes aboard, ore by drone ([drone-mining.md](drone-mining.md)) |
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
- **The eight-material showroom ring**, moved into a room of its own in N1's commons (decided 2026-11-06: "Let's try it.").
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
- **His garden**: 1,976 crops, 1,575 of them dead (80%). **Wiped with the rest** (decided
  2026-11-07: "Wipe it."). The showcase replants itself whenever a Dev or Creative world has no
  crops (data/world/showcase.ron), and a new homestead starts with the comfortable garden
  ([survival-garden.md](survival-garden.md), decided the same day).
- **His installed data folder** (`%APPDATA%\HumanityOS\data`, stamped v0.1525.0): the refresh
  replaces every file nobody edited with the new version and KEEPS one he saved from the editor
  (crates/humanity-core/src/storage/refresh.rs). Before the wipe ships, check Settings > Data for
  a kept ship file, home design or machine file; "Use the game's copy" moves his aside.

### 6.4 Rung 1: Neighbourhood 1, flat

The smallest correct slice the game can stand on. It is a rung of the real thing: the layout,
the coordinates and the relations are Drum 1's, and N1 is Sector 1's first neighbourhood (3.15);
only the curvature waits for rung 2.

**The frame.** Drum 1's floor, unrolled: x is the distance around the drum (s), z the distance
along it from the fore end cap (a), y up from the floor. N1 is the cell s 0 to 393, a 120 to 420,
at the fore end of Sector 1 (3.4); rung 3 re-cuts it into the sector's half-ring template. In rung
2 one transform rolls every point: angle s / 250 m, radius 250 m minus y.

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
   exhibit room (decided 2026-11-06); the guest's arrival point in the open. Its machines on the ship's
   own meter.
5. **The hologram** placed by the home design (a fixture row in the study or the console room).
6. **No districts, no wild nodes, no fixed quest coordinates aboard.** The Survey quests read their
   ore from the drone mining loop (decided 2026-11-06; [drone-mining.md](drone-mining.md)).
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
| 2. The drum | Roll Drum 1's floor: the transform (f64 on the CPU, CLAUDE.md's planet-scale rule), up is radial, homes rolled with it (walls radial, floors following the curve), the spin as a frame (the planet surface walk already takes `up` as an argument, habitat-generation.md:131-158), the far side drawn (L2), the sunline lights the floor on one shared day (3.13: 250 lux melanopic EDI at the eye by day, 10 in the evening, 1 in bedrooms at night) | the transform round-trips within 1 mm at 250 m; a walk around the whole circumference returns to its start; footing on a rolled floor at every storey; the light rules over every home's rooms | a new `verify-drum` rig (walk 100 m spinward and back, up vector and footing judged); vantages `drum-floor-noon`, `drum-far-side` |
| 3. Sectors and neighbourhoods as data | the sector template (3.15: a half ring of three neighbourhoods, its civic heart, farm band and workshops), neighbourhood templates, the generator, stable plot ids, plots that turn, Sector 1 whole and the next sectors of Drum 1, apartment blocks drawn from `data/ship/blocks/` | the planning rules over the generated layout (green space within 300 m, 3 trees in view, 30% canopy, the walking limits: 3.13); the layout's areas within the space program's (3.12); ids stable under regeneration; no overlap | `verify-copresence` with a dozen players (the co-op floor) |
| 4. Arrival | the bow, Hangar Bay 1 as a walkable zone, the Arrivals hall and desk, the arrival lift down the fore end cap, General Services at the Fore Landing, the transport (game-setting-and-intro.md rungs 2 and 3) | the arrival path is floor end to end; every desk step reachable; respawn rows resolve | a new `verify-arrival` rig |
| 5. Engineering and the ship's supply | the spine's engineering segment, ship machines and lights on the ship's meter and ledger, hum sources at the engineering segment and the stern (`data/audio/hum_sources.ron`), the noise rule reading the same rows (each with its sound power and its room's wall loss), air zones | no ship machine on a household meter; every bedroom inside the night noise rules of 3.13; no home in an industrial air zone | `verify-runtime`; the hum level read at three vantages |
| 6. Farms and freight | the farm band as real machines filling the mess halls' stores (replacing the stand-in "36 meals a game day"), parcel lockers, freight lanes, the Aft Landing depot, the spine freight rail | a meal's food traces to a farm row; freight never shares passenger paths | `verify-starts` (a cabin eats from the farms) |
| 7. Civic | the Concourse, the Medical Bay, the greenbelt (park, lake, arena field, school and Library hall) | 15 minutes to every daily need | vantages and a walk rig |
| 8. The exterior | the bow, the sectors, the stern drive, the radiators, the drums turning, at the size question 1 settles; the hull wrap retired | the exterior is built from the sector rows | `verify-runtime` exterior vantages |
| 9. MMO scale | claim order across neighbourhoods and sectors, interest by neighbourhood, crowd tiers 1 to 3 | per-frame cost flat as population grows (crowd-simulation.md); no NPC household above its home's design occupancy (3.13) | `npc-crowd-stress` measurements |

Drone mining has rungs of its own ([drone-mining.md](drone-mining.md) section 10). Its rungs 0 and
1 (distances, propellant and hold in the automated loop) should land with this plan's rung 1, so
the Survey quests, re-pointed from the wild nodes to the drones, have a working ore source; the
bow's drone bay and ore intake arrive with rung 4 (arrival builds the bow).

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
| The ship's hum (music.md): `data/audio/hum_sources.ron`, positions in ship metres, the first drive source at "the ship's stern" | not yet built; if it lands before the wipe, its stern is today's slab's end past p12 | rung 1 keeps one engine source at Drum 1's aft end (axial 2,000 m, the spine) so the hum already rises toward the Aft Landing; rung 5 adds the engineering segment's sources |
| The Maps page's ship symbol and its rigs (scripts/lib/map-ship-check.js and the others) | the ship as one symbol in orbit | nothing in rung 1; rung 8 (the exterior) |
| Quests: Initial Survey, Distant Expeditions, the explore quest, getting_started | ore from the wild outcrop and vein; fixed places; "find your home" | rung 1 re-points the ore to the mining loop and the places to anchors (question 8) |

## 7. Questions for the operator

Each with a recommendation. The wipe itself waits for his read of this plan. His answers of
2026-11-06 are in section 1.1 and of 2026-11-07 in section 1.2; a question marked DECIDED is
settled, one marked OPEN keeps its recommendation until he answers.

1. **How big is the whole mothership, and how many live aboard?** DECIDED 2026-11-07: aim for 1
   billion and 10 billion, in sectors of 500 ("Let's start with a sector for 500 people."). The
   research on 500 is in 3.15. OPEN, the consequence: the honest answer is a **fleet**, about 204
   ships for a billion and 2,033 for ten billion, each 332 km long with 246 drums of 1 km radius
   (3.16). *Recommendation:* accept the fleet (one spine cannot carry them), with the bulk of the
   population in 1 km drums spinning at 0.95 rpm and Drum 1 kept at your 250 m as the first.
1b. **Drum 1's length against spin stability** (new). A cylinder flying free spins stably only up
   to 1.3 radii long; Drum 1 is 8 (3.2, 3.16). *Recommendation:* keep its radius and its 2 km,
   built as a row of short drums of one ring each (263 m, two sectors), each stable on its own,
   turning in pairs, with active damping on the spine as well; the other choice is one 2 km drum
   held by active control alone, which the sources name but which fails dangerously if the control
   fails (a slow tumble end over end).
2. **Cylinder or hyperboloid for Drum 1?** DECIDED 2026-11-06: a plain cylinder.
3. **Where does a home's light come from inside the drum?** DECIDED in part 2026-11-06: the
   sunline ("I look forward to seeing it."). OPEN: piped sunlight at a star, lamps between the
   stars, which follows "is the fleet at Earth or between the stars" (game-setting-and-intro.md
   question 2). *Recommendation* as before: glass roofs and the homestead's solar panels read the
   sunline's light, and between the stars the ledger shows that every panel watt was a reactor
   watt first.
4. **One deck or many for homes?** DECIDED 2026-11-06: one deck, the drum floor; one homestead a
   plot, never stacked; homes may have their own storeys; apartments stack around an atrium (3.6).
5. **What do we keep of today's home?** OPEN. *Recommendation:* the homestead design whole (your
   2026-09-19 redesign and the upper floor) as every homestead's template; drop the train
   platform, its ramp and the teleporter pair from it (transit belongs to the ship); a respawn pod
   beside the bedroom when respawn-and-revival.md settles how waking works.
6. **Your garden and save.** The garden: DECIDED 2026-11-07, "Wipe it." The rest OPEN.
   *Recommendation:* keep your character (name, look, outfit, skills, credits, inventory,
   quests); move your car home.
7. **The old walls in the Commons.** DECIDED 2026-11-06: the showroom ring to its own exhibit room,
   the room block retired.
8. **Berry bushes, salt flats and ore veins aboard?** DECIDED 2026-11-06: none aboard; ore from
   distant asteroids by drone ([drone-mining.md](drone-mining.md)); fibre, berries and wood from the
   ship's farms and orchards; the Survey quests change to match.
9. **The districts.** DECIDED 2026-11-06: all fourteen out now, each back as a real walkable place
   in its rung; realism the standard.
10. **Cruise thrust.** DECIDED 2026-11-07: a mechanic with no thrusting, the jump ("Like a
    teleportation tech requires no thrusting."), named as made up; thrust only to manoeuvre, at
    most 0.05 g, routinely 0.02 g; floors fixed (3.14).
10b. **Do the motherships now jump?** (new) Your 2026-11-07 answer replaces the 2026-11-03 "small
    craft jump, the motherships never do" (game-setting-and-intro.md 10.1). *Recommendation:* yes,
    the whole fleet jumps between stars, small craft and drones jump short distances from it, and
    game-setting-and-intro.md's question 1 is re-read with that.
11. **How far must homes be from the loud places?** DECIDED in direction 2026-11-06 ("We want to
    maximize crewmember health."). The rule is computed, not a fixed distance: every bedroom under
    40 dB outside and 30 dB inside at night from all sources together (3.13).
12. **Neighbourhood 1's mix.** DECIDED 2026-11-06: homesteads and the cabin block side by side.
13. **The mix of homes.** OPEN. At 8% homesteaders Drum 1 holds about 7,800; at no homesteads
    about 11,500; at all homesteads about 1,200 (3.12). A sector of 500 at 8% has 16 homesteads.
    *Recommendation:* keep 8% as the default and let each server set its own mix in data, so a
    server that wants more homesteads opens its next sector sooner.
14. **One level of farms under the sunline, or two under lamps?** DECIDED 2026-11-07: "Let's start
    with one but, allow for two or more." (`[farms] levels`, 3.12.)
15. **The bow's room for private craft.** The word: DECIDED 2026-11-07 (a "cradle" was a parking
    spot for one craft, not a way of reviving people; it now says parking spot). The number: OPEN.
    One parking spot for every 20 people fills the proposed bow (120 m across, 300 m long) at
    about 11,500 people (the spot is 200 m2 since 2026-11-13, a real four-seat craft with its clearance). *Recommendation:* keep one for every 20 people for Drum 1, and give later
    drums hangar decks of their own in their spine hubs rather than one ever-growing bow.
16. **Drone mining.** DECIDED 2026-11-07: drones jump only to discovered nearby asteroids, comets
    and the like, never between stars ("Maybe not jump between stars but, at least jump to
    discovered nearby asteroids/comets/whatever."). OPEN: where the drone is kept.
    *Recommendation* as before: in the bow's drone bay, the home's drone hangar becoming the console
    you fly it from ([drone-mining.md](drone-mining.md) sections 2, 4 and 8; its open questions in
    section 12).

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

Added with his answers (2026-11-06): the findings
[2026-11-06-space-program-sources.md](../reference/findings/2026-11-06-space-program-sources.md)
(NASA SP-413's space program, the GSA's circulation guide) and
[2026-11-06-noise-and-sleep.md](../reference/findings/2026-11-06-noise-and-sleep.md) (the WHO's
noise, green space, housing and air guidelines, NASA's spacecraft noise limits, light at the eye);
[drone-mining.md](drone-mining.md); [music.md](music.md) (the hum); the data
[data/ship/space_program.toml](../../data/ship/space_program.toml) and
[data/ship/blocks/apartment_atrium.toml](../../data/ship/blocks/apartment_atrium.toml), read by
`scripts/space-program.js`.

Added with his second answers (2026-11-07): the findings
[2026-11-07-community-size.md](../reference/findings/2026-11-07-community-size.md) (Dunbar's layers,
Hutterite and Alpine community sizes, the 500 threshold, the 2021 critique and the reply) and
[2026-11-07-drum-limits.md](../reference/findings/2026-11-07-drum-limits.md) (hoop stress, the
1.3-radius spin-stability rule, steel, aramid and carbon composite strengths, the main belt's mass,
the ADA's floor slopes); [survival-garden.md](survival-garden.md) (the comfortable garden the
farms now grow); game-setting-and-intro.md 10.1 (the 2026-11-03 jump rule this plan's 3.14
changes).
