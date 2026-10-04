# Ship homes, plots and moving goods

**Status, 2026-10-03.** The operator decided the direction on 2026-10-03. Everything in this document about *how* to do it is an agent proposal. It was judged from three proposals (engineering build order, the player's view, logistics and city planning) and four read-only surveys done the same day. Nothing in this document has been built or booted.

How claims are marked:
- Repo facts cite `file:line`.
- World facts cite a link and the source's own date.
- "Computed" means arithmetic on cited figures.
- "Assumption" means a number of ours that data should replace later.

**What the operator decided on 2026-10-03, in his words:**
- "I would actually like the player homes in multiplayer to actually have their own locations."
- "on the MMORPG game mode there technically won't be any 'main player' but, a bunch of equal tier players."
- The starts: "the default full homestead with everything"; "a like apartment complex where players have a smaller home and several areas are shared with the group, like the mess hall"; "a bare minimum player style, like for kids or people with just not enough time to deal with the farming stuff."
- "I'd like players trading goods have to actually have the goods transported either by the player or an automated robot or some other method." Goods going to a different mothership take longer, and "That's why we also want trains, elevators, and other fast travel methods for getting around."
- "I'd like for player build area to be limited to their specific home. I don't like the idea of homes overlapping."
- "Players could meet anywhere on the mothership ideally. The mess hall, recreation room, crafting areas, hangar where all the private spaceships are stored, or whatever."
- "Embarking on certain missions could require grouping up in the hangar at a specific transport ship waiting for players."
- "We really want it to logistically make sense. Theoretically HumanityOS could be used for city planning."

**What he asked and left open:** "What starts do you think we could/should have?" and "I don't know what the proper terms or tiers would be." Section 1 answers both as proposals.

**His follow-ups the same night (decided):**
- Food for cabins and apartments is still physically accounted for: they eat from communal gardens and the ship's farms instead of tending their own, "kind of how homes in real-life are since most aren't homesteads"; a suburban house is limited to what its lawn can do. Section 1.3 already has this shape; it is now his decision, not a proposal.
- He is "fairly happy" with Cabin, Apartment and Homestead, asked which other styles exist, and then chose the FULL list: "I like the full list as it adds variety and allows for a full spectrum of colony spaceship designs." So every home type in section 1.2b is a data-defined kind, not just three starts.
- The full homestead is the default start ("the default full homestead with everything").

**Proposed here, not decided:** the names and contents of the starts, plot sizes, the neighbourhood layout, carrier speeds, the server tables, the order of increments, and every item in section 9.

**Earlier operator decisions this builds on:**
- The ship is premade. A player changes only "their specific home area in the spaceship" (docs/design/the-mothership.md:20-24, 2026-09-19).
- Editing in normal play is "confined to their default zone" (docs/design/game-modes.md:67-68, 2026-09-15).
- Co-op must "support at least a dozen" (docs/design/game-modes.md:260).
- The reactor should "essentially provide unlimited (at least to start)", metered (docs/PRIORITIES.md:453, 2026-09-27).
- No FTL (docs/design/gravity-and-movement.md:322-344).

---

## 0. In short

- **A full spectrum of home types, all with equal standing, every one a data file** (the operator chose the full list, 2026-10-03; section 1.2b). The three below are the core; Townhouse, House, Smallholding and Farm fill in between, and group forms (housing co-op, commune) are ways a block or a set of plots is run. The Homestead is the default start.
  - **Cabin** is the minimum: a private room; you eat at the mess hall and grow nothing.
  - **Apartment** is a small home in a block that shares a common house with a mess hall. The real-world term is cohousing.
  - **Homestead** is the full acre we have now.
  - A **group farm** is something homesteaders grow into later. It is never a start you pick.
  - A start sets how much space you hold and how much upkeep you carry. It never sets rank or how deep the simulation runs.
- **Homes are plots in one ship frame.** Plots never overlap. A player builds only inside their own plot. The relay assigns plots, and each player spawns at their own.
- **People meet in the ship's shared places.** Those places draw people because they do things a home cannot.
- **Missions gather in person** at a transport in the hangar, which leaves on a timetable.
- **Goods always sit in exactly one place:** a locker, someone's hands, a carrier, or a shop. Moving them takes real seconds at real speeds. Goods travel without you, so nobody waits at a menu.
- **The relay holds four new things:** who owns each plot, holdings, shipments in transit, and departures. Geometry, starts, carriers and lines live in data, shaped so the same model could load a real neighbourhood.
- **The first increment this week:** two players, two homes, each spawning at their own plot, proven by `just verify-copresence` in both join orders.

---

## 1. The starts

*Taken from:*
- the name Cabin, the UI words and the household rule: Proposal 2;
- floor areas and cohousing figures: Proposal 3;
- "starts, not tiers": all three;
- the group farm: Proposals 2 and 3.

*Contradictions resolved:*
- **The minimum start's name.** Berth (Proposal 1), Cabin (Proposal 2) or Quarters (Proposal 3). **Cabin.**
  - "Quarters" already names the rooms.ron type for *shared bunks* (data/rooms.ron:433-443) and the relay's Pioneer room `quarters` (src/relay/handlers/game_state.rs:897).
  - "Berth" is what docs/design/room-purpose-architecture.md:102 calls a home's id.
  - Either word would then mean two things. A cabin is the ship word for a private room, and children know it.
- **The middle start's name.** On screen it is **Apartment**, in a **block** with a **common house**. "Cohousing" is the accurate real-world term and is used in the design and in the Library page that explains it (Proposal 3 suggested the same fallback).
- **The Cabin's washroom.** Proposal 2 gave each cabin a private wet room, about 16 m2 in all. Proposal 3 shared washrooms per block of 40 or fewer.
  - This document recommends shared washrooms: that is how real berthing works, it saves floor area, and it gives the block a place where people cross paths.
  - It is open question 2.

### 1.1 The rule underneath

- **No start changes how deep the simulation runs.** The engagement design rejected that: "One simulation = one code path = far fewer failure modes than a tier matrix" (docs/design/engagement-modes.md:43).
- **A start is two things:** a home size, and a default engagement mode for each need. Modes are chosen "per-domain (water, food, fertilizer, energy), not globally" (engagement-modes.md:39). A player can change any of them later.
- **Difficulty is a separate axis,** chosen per world: no-fail or Hardcore (docs/design/game-modes.md:102-124, 245-253). A child can live in a Homestead on a no-fail world, and an expert can pick a Cabin.
- **The screen never says "tier".** The word reads as rank. The picker says **"Choose where you live"**, and changing later is **"Moving house"**.
- **Starts belong to a household, not a person.** The existing Family/Solo choice (src/config.rs:716-717, src/gui/pages/settings.rs:3585-3602) stays as the household-size dial inside any start, so you can have a Family Apartment or a Solo Homestead.

### 1.2 The three starts

| | **Cabin** | **Apartment** (in a block) | **Homestead** |
|---|---|---|---|
| Real-world term | crew quarters, berthing | cohousing (Danish *bofaellesskab*) | homestead (US), smallholding (UK) |
| Private | one room: bed, locker, desk, wall screen | small home: bedroom(s), bathroom, kitchenette, living space | the full acre: house, greenhouse, barn, fields, workshop, forge, vehicle bay, power terrace |
| Floor | 7.5 m2 single, 11.5 m2 double | 37 to 70 m2 | 55 x 89 m = 4,895 m2 |
| Shared | washrooms per block, mess hall, recreation, laundry, workshops, everything else | the common house (mess hall, big kitchen, dining, laundry, recreation, workshop, kids' and guest rooms), block garden, small animal pens | the ship's public places |
| Food | mess hall (Background mode; better meals by Trade or a crew shift) | Cooperative: common dinners on a rota, or cook at home | Direct |
| Energy, water | metered, pure consumer | metered, pure consumer | Direct (own power terrace) |
| Upkeep | nothing at home can fail | your small home plus optional rota shifts | real work on crops, animals, machines and power |
| You build | furnishing inside your cabin | inside your home | anything inside your acre |
| Suits | children, short sessions, newcomers, people who come to socialise or fly missions, life-tool users | busy adults, friend groups, families who want company | dedicated players, families who play together |

**Cabin.**
- **Size:** the UK space standard sets a single bedroom at "no less than 7.5 m2" and a double at 11.5 m2 ([Nationally Described Space Standard](https://assets.publishing.service.gov.uk/media/6123c60e8fa8f53dd1f9b04d/160519_Nationally_Described_Space_Standard.pdf), published 2015-03-27, notes added 2016-05-19).
- **Privacy at the smallest size:**
  - An ISS crew quarter is 2.1 m3 ([Wikipedia, Habitation Module](https://en.wikipedia.org/wiki/Habitation_Module), edited 2026-03-27).
  - NASA lists "sensory stimulation, communication, autonomy, and privacy" as habitat design factors for long missions ([NTRS 20160014500](https://ntrs.nasa.gov/citations/20160014500), 2016-12-04).
  - So the Cabin is private and quiet, and everything else is shared.
- **Washrooms:** shared per block of 40 cabins or fewer. The Ford-class carrier moved to "40-man or smaller berthings to reduce noise and distractions", each with "an associated head" ([Navy Times](https://www.navytimes.com/story/military/tech/2014/10/13/crews-ship-sailors-comfort-a-centerpiece-of-new-supercarrier-ford/17180325/), 2014-10-13).
- **Food:** the player is crew ("The player just happens to be one of the crew members", docs/design/the-mothership.md:15). The mess hall serves a free, metered basic crew meal. That matches the reactor decision, and engagement-modes' rule that Background always shows its upkeep.
- **Utilities:** grid-hierarchy already allows "A player CAN choose to be a pure consumer" (docs/design/grid-hierarchy.md:45).
- **A first step into growing:** a window-box plant is the optional first Direct step.

**Apartment.**
- **Home size:** 37 to 70 m2, from the same standard's one-bedroom one-person flat with a shower (37 m2) to its two-bedroom four-person flat (70 m2).
- **Block size:**
  - Real cohousing is "usually limited to around 20-50 homes" ([Wikipedia, Cohousing](https://en.wikipedia.org/wiki/Cohousing), edited 2026-08-23), or "around 40 to 100 people" (McCamant and Durrett, [In Context](https://www.context.org/?p=634), Spring 1989).
  - The first block should be 12 to 30 homes. That seats the operator's dozen as one block. The number goes in data, not code.
- **Common house size:** common houses "average 2,000 ft2 (186 m2) to 8,000 ft2 (743 m2)" (Coldham, ["Cohouses Are Smaller"](https://www.buildinggreen.com/node/11978), 1999-03-01, citing Meltzer 1996).
- **Food:** "Common dinners, prepared by small teams on a rotating basis, have proven enormously popular" (In Context, Spring 1989).
- **Upkeep:** rota shifts are how a resident contributes. A missed shift is metered and shown, never punished. That is the operator's utilities rule applied to food: "we help them understand how much they consume" (docs/design/grid-hierarchy.md:8).
- **Lessons from games:**
  - Shared space must not belong to one leader. WoW's garrison designer said the guild master would have fun "and everybody else would just show up on occasion" ([PCGamesN](https://pcgamesn.com/wow/why-world-warcraft-warlords-draenors-garrisons-arent-guilds), updated 2017-08-17).
  - Every member must be able to move shared projects forward. Animal Crossing's second player became "a glorified visitor" ([Kotaku](https://kotaku.com/it-sucks-being-player-2-in-animal-crossing-1842477024), 2020-03-24).

**Homestead.**
- **What the plot is:** the shipped acre, 55 x 89 m, 1.21 acres, 23 rooms in bands along z (data/blueprints/ship_structure.ron:28-40).
- **Growing space,** computed from those bands: about 2,270 m2. That is the greenhouse 990, fields 640, plant room 252, mushroom and aquaponics room 220, and orchard court 168.
- **The real word:** "A smallholding is a small farm operating under a small-scale agriculture model", typically supporting "a single family" ([Wikipedia](https://en.wikipedia.org/wiki/Smallholding), edited 2026-08-22).

**The group farm** is something players grow into, never a start.
- Neighbouring homesteaders join their plots by a signed agreement: the same certificate as a build permit (section 4). The plot rows do not change.
- This is the operator's own instinct: "requiring users to join groups to form larger farms than just the 1 acre" (docs/design/habitat-generation.md:291-292, 2026-09-15).

### 1.2b The full list (operator decision, 2026-10-03)

The operator asked which other home styles exist and chose the full list, "as it adds variety and allows for a full spectrum of colony spaceship designs." Every row is a home kind in data (`data/homes/*.ron`, increment 6), so a ship designer can mix them, and new kinds need no code.

| Kind | Real-world term | Private | Food |
|---|---|---|---|
| **Cabin** | crew quarters, berth, dormitory | one room | all from the commons |
| **Apartment** | flat, studio; **cohousing** with a common house | a small home in a block | mostly commons, maybe a balcony planter |
| **Townhouse** | row house, terraced house | a narrow home sharing side walls, a small private yard | a few raised beds, the rest bought |
| **House** | suburban detached house | a house with a yard or lawn | a kitchen garden or a fruit tree at most; "limited to what it can do with its lawns" (the operator) |
| **Smallholding** | hobby farm | a large plot | much of your own food, some surplus |
| **Homestead** (default start) | homestead, smallholding at full self-sufficiency | the full acre (section 1.2) | all your own |
| **Farm** | a working farm | land worked for others | feeds part of the ship and sells to the commons |

Group forms are ways of RUNNING homes, not home kinds: a **housing co-op** (residents own and run the block together), a **commune** or **kibbutz** (shared work and shared food), an **intentional community**. They map onto the block and plot rows as data (who decides what, who shares what), and onto the group farm below.

Whether every kind is offered at the first "Choose where you live", or some are reached by moving house, is open question 1. The recommendation: offer all of them, with the Homestead highlighted as the default and one plain sentence per kind saying what it asks of you.

### 1.3 Why the starts need each other: food land

Every resident needs growing area somewhere on the ship, whatever their start.
- Ecology Action claims 2,800 to 3,403 ft2, which is 260 to 316 m2, for "a complete year's diet for one person" ([growbiointensive.org](https://growbiointensive.org/About_highlights.html), no page date).
- docs/design/self-sufficiency.md:82 gives 700 to 1,000 m2 with no source. Reconcile the two in a dated findings document.

What follows from that:
- Cabins and Apartments do not remove farmland. They move it out to the ship's agriculture zones, and the freight network has to carry the food.
- A Homestead's 2,270 m2 feeds about 2 to 9 people (computed).
- Its surplus stocks the mess halls. Cabin and Apartment residents give back in work, crafts, hauling and services.
- That is "A player who hates gardening buys food with mining credits" (docs/design/gameplay-loop-map.md:190) made physical.
- Eco shows players accept this shape: "you achieve the most when you specialize and trade both goods and services" ([Steam](https://store.steampowered.com/app/382310/Eco/), fetched 2026-10-03).

### 1.4 Day one and a month later

- **A child in a Cabin.**
  - Day one: they wake in their cabin, and the screen says breakfast is in the mess hall, a 4 to 6 minute walk. The recreation room has games and shared screens. Nothing at home can die.
  - A month later: the cabin is decorated with things they found or made, a window-box plant grows, and they ride the train alone. They lost nothing in the week they did not play.
- **A busy adult in an Apartment.**
  - Day one: they meet the block at tonight's common dinner and pick one way to contribute (a Thursday cooking shift, a market stall, courier jobs).
  - A month later: the block covers their food, and parcels arrived while they were offline. They have a regular hangar-mission group. If the block garden hooked them, they can move to a Homestead; if life got busy, to a Cabin. They keep everything either way.
- **A dedicated homesteader.**
  - Day one: an acre with seed stock and tools, and metered reactor power. The first jobs are planting, water and a meal cooked at home. The mess hall is the backstop, so a bad first week cannot starve them.
  - A month later: harvests come in. How many depends on the open clock decision: at 1x, "a lettuce take[s] 45 days" (docs/PRIORITIES.md:119). Surplus goes by freight to mess halls and the market.

### 1.5 Moving between starts

- **When:** a free choice, any time a home of that kind is free. A cooldown (a Server Settings row) stops daily hopping.
  - Animal Crossing's loans carry no deadline ([Nookipedia](https://nookipedia.com/wiki/Home_loan), edited 2026-03-12).
- **Moving costs time, not progress:**
  - Your locker and placed items are packed and sent as a shipment (section 5). That makes moving day the first real job of the logistics system.
  - Moving to a smaller home never destroys anything. What does not fit goes to a storage unit.
  - The old plot returns to the pool.
- **One home per player per mothership.**
  - In FFXIV two players bought 28 homes, and the fix became "one personal plot each" ([Kotaku](https://kotaku.com/final-fantasy-xivs-director-talks-housing-shortage-ps4-1827024591), 2018-06-21).
  - FFXIV allows "one apartment per character" ([wiki](https://ffxiv.consolegameswiki.com/wiki/Housing), edited 2026-07-27).
- **When a kind of home is full:** a first-come waiting list, never a lottery.
  - FFXIV's lottery was "the best that the team could come up with" ([Destructoid](https://www.destructoid.com/square-enix-clarifies-the-upcoming-final-fantasy-xiv-housing-lottery/), 2022-04-01).
  - WoW's housing promises "no lotteries, and no onerous upkeep" ([Blizzard](https://worldofwarcraft.blizzard.com/news/24230692), dated "September 2nd"; the year was not shown, and other coverage places it in 2025).
- **Absent owners: never demolish.**
  - FFXIV demolishes a house not visited in 45 days (wiki above), and that is the most disliked part of its housing.
  - Here, a home is mothballed only when the ship is full, someone is waiting, and the owner has been away a long time. Its contents go into storage, and the owner gets the next free home of that kind on return.
- **Children:** the picker offers "Cabin + simplified mode" as a suggested pair. They stay two separate settings, because the start decides what you own and the mode decides how hard the simulation pushes back.

---

## 2. Homes on the ship

*Taken from:*
- the frame, the plot record, the first test layout and the planning rules as data tests: Proposal 1;
- drum numbers, capacity and the walking-time sizing: Proposal 3;
- placement by density: Proposal 2 (transit-oriented development) and Proposal 3 (von Thünen).

*Contradictions resolved:*
- **Where plot geometry lives.** Proposal 1 keeps it in data with only ownership in the database. Proposals 2 and 3 put geometry columns in the database. **Data:**
  - one source of truth;
  - hot-reloadable;
  - the same file the client draws from;
  - a database row whose plot has left the data is simply dropped.
- **How neighbourhoods are sized.** Proposal 3 uses walking time. mothership-superstructure.md:68-86 uses Dunbar layers with no source, and a 2021 reanalysis found "95% confidence intervals (4-520 and 2-336)" for Dunbar's figure ([Wikipedia](https://en.wikipedia.org/wiki/Dunbar%27s_number), edited 2026-09-10). Walking time can be measured, so it wins.
- **Nesting.** Cabins and Apartments are child plots inside a block plot (Proposal 3's parent id). Block membership then comes from where your home is, never from a member list (all three agree).

### 2.1 What the code does today, and why homes overlap

**There is one home, and it sits at the origin.**
- "Home" is the zone whose id is `home` (src/ship/ship_structure.rs:652-654).
- World load uses the home's authored spawn as a *world* position and never adds the zone origin (src/engine/world_load.rs:115-120, 199-202).
- Machine x and z are "ABSOLUTE world metres, clamped into the named zone's footprint" (src/machines.rs:553-558). Moving a home's origin would pile every machine against the box edge.

**The mothership plan lives inside the player's home.**
- The 13 districts (residential, hangar, mall, transit and the rest) are sub-zones of the home body (ship_structure.ron:1653-1741).
- The 11 Commons machines are rows in the player's own data/machines/home.ron.

**Nothing prevents overlap.**
- `validate()` checks ids and corridors only (ship_structure.rs:252-275).
- The only allocator places a new zone in a row along +X (ship_structure.rs:684-692).
- The render-only neighbour clones in `res-1` (origin 57,0,0, 120 x 200 m, ship_structure.ron:1653-1658) already overlap the Commons (65,0,20, 34 x 55 m, ship_structure.ron:1749-1752) and the corridor between them.

**The relay lives in another ship.**
- A fresh join spawns at [0,1,0] (src/relay/handlers/msg_handlers.rs:2981-2983). spawn_player turns that into the Pioneer's Crew Quarters centre (game_state.rs:585-589, 894-903): room (0,4,7), size (8,3.5,6) in data/ships/starter_fleet.ron, so the point is (4, 5, 10).
- An update more than 100 m from the stored position is refused, and the stored position is not moved (msg_handlers.rs:3285-3296).

**Other single-home assumptions:**
- "aboard" is a 400 m sphere (src/lib.rs:3424);
- collision ignores height (src/ship/wall_collision.rs:1-11);
- there is one 14,000 m3 air volume (src/engine/home_spawn.rs:62);
- there is one `PLAYER_HOME` (src/systems/ship_power.rs:43).

**Anyone in a shared world can edit the ship.** The shipped PlayMode defaults to Dev (src/config.rs:130-131), and ship editing is Dev-only (config.rs:170).

**Already in place and reused:**
- Clients send their camera position in the ship frame, and draw others at that position plus `station_off`, the same offset as home content (src/engine/net_route.rs:262-283, 375-380). A shared ship frame needs no new maths on the wire.
- Moving a zone's origin and rebuilding is an existing editor path (src/gui/pages/construction.rs:1712-1745, then src/engine/home_meshes.rs:206).
- `ship` and `machines` compile into the relay (src/lib.rs:18-21; only `engine` is native-only, lib.rs:66-67).
- The co-presence rig (scripts/verify-copresence.js), the scripted player (scripts/second-player.js), and the remote-player recorder. The recorder logs each remote player's *drawn* position every frame, whether or not it is on screen (src/engine/ipc.rs:2956-2962).

### 2.2 The ship frame

- **One frame per mothership:** metres, Y up, with the origin at today's home corner. Plot `p1` is exactly today's home, so nothing moves.
- **Precision:** within ±2 km, f32 resolves about 0.12 mm (2^-13 m spacing between 1,024 and 2,048 m), which is fine for one ship. Never use one frame for a fleet (the f32-at-planet-scale rule in CLAUDE.md).
- **Plot frames are local.** A plot's place in the ship is its origin, plus a quarter-turn yaw later.
  - Zones have no rotation today, and the homestead's door is in its east wall (ship_structure.ron:28-41). So the first street has homes on one side only.
  - On the drum, the ship frame is the unrolled interior sheet. Curvature changes only the plot-to-ship transform, never plot-local coordinates.

### 2.3 Plots

**A plot record** (data, stored with the ship):

| Field | Meaning |
|---|---|
| `id` | stable, never a list index |
| `parent` | the block plot, if any |
| `kind` | which starts and designs fit |
| `origin`, `size` | where the plot sits and how big it is |
| `door` | the street zone id and the lateral position of the door along it |
| `neighbourhood` | which neighbourhood it belongs to |

**Rules, checked by `validate()` and a data test:**
- no plot overlaps another plot, a shared zone or a corridor tube;
- a child plot lies inside its parent;
- the player builds only inside their plot's box.

**Plot sizes for each start:**

| Start | Plot | Source |
|---|---|---|
| Cabin | 7.5 m2 single, 11.5 m2 double, inside a block plot of up to 40 cabins with shared washrooms. The block's corridor and washroom share is set by walking it in the rig (assumption) | NDSS 2015; Navy Times 2014 |
| Apartment | 37 to 70 m2, inside a block plot of 12 to 30 homes (later up to 50) around a common house of 186 to 743 m2 | NDSS; Wikipedia Cohousing 2026-08-23; Coldham 1999 |
| Homestead | 55 x 89 m = 4,895 m2 | ship_structure.ron:28-40 |
| Group farm | adjacent Homestead plots joined by certificate | habitat-generation.md:291-292 |

The operator called the acre "whatever it is for that specific ship design" (the-mothership.md:24). So plot sizes are per ship in data, not constants.

### 2.4 The first test layout (increment 1)

Worked out from the shipped numbers:
- **p1** at (0,0,0), 55 x 89 m. This is today's home. Its corridor to the Commons is unchanged, at lateral position 40 (ship_structure.ron:2656-2665).
- **street-1** at (65,0,85), 10 x 110 m, covering x 65..75 and z 85..195.
- **p2** at (0,0,99), 55 x 89 m, covering z 99..188: a 10 m gap south of p1. Its corridor to street-1 runs along x at lateral position 139, because its door sits at local z 40, the same as p1's.
- **Commons to street-1:** a corridor along z at lateral position 70. The Commons ends at z 75, the street starts at z 85, and both cover x 65..75.

Both new corridors meet the corridor rules: straight, level, with a clear gap on one axis (ship_structure.rs:280-360).
- p2 to street: the gap is x 55..65, and z overlaps across 99..188.
- Commons to street: the gap is z 75..85, and x overlaps across 65..75.

**Spawns, and why p2 is the failing case:**
- p2's spawn is (53.5, 1.7, 139.5), from the authored point (53.5, 40.5) at ship_structure.ron:1420. That is 138.7 m from the Pioneer spawn (4, 5, 10), so today's relay refuses that player's first update.
- p1's spawn is 58.2 m from it.
- Everything stays inside the 400 m "aboard" sphere.

### 2.5 Neighbourhoods and districts

| Level | Rule | Size | Source |
|---|---|---|---|
| Plot | one household | 7.5 m2 / 37-70 m2 / 4,895 m2 | NDSS; ship_structure.ron |
| Block | homes around a common house or shared washrooms | 12-50 homes, 40-100 people | [Cohousing](https://en.wikipedia.org/wiki/Cohousing), 2026-08-23; In Context 1989 |
| Neighbourhood | daily needs within about 400 m, a 4 to 6 minute walk | Perry: "a child's walk to school is only about one quarter of a mile"; 5,000 to 9,000 residents on 160 acres on Earth; at least 10% parks | [Neighbourhood unit](https://en.wikipedia.org/wiki/Neighbourhood_unit), 2026-09-26 |
| District | everything daily within 15 minutes | 1,260 m at 1.4 m/s (computed) | [15-minute city](https://en.wikipedia.org/wiki/15-minute_city), 2026-10-03 |
| Ship | rail, elevators, tubes | the drum plus the spine | |
| Fleet | shuttles | other motherships | |

- **Walking speed:** design guides use 1.4 m/s, and observed speeds run 1.10 to 1.65 m/s ([Preferred walking speed](https://en.wikipedia.org/wiki/Preferred_walking_speed), edited 2026-07-27). So 400 m takes 4.0 to 6.1 minutes (computed).
- **A game precedent:** WoW neighbourhoods hold "roughly 50 plots of the same size" (Blizzard, above).
- **Each neighbourhood is built around one node:** a transit stop, a common house or mess hall, a clinic, a school or Library room, and a recreation room.

### 2.6 The first drum, worked through

All computed from the 250 m radius by 2 km drum (habitat-generation.md:18-29).

- **Shape:** the interior unrolls to a sheet 2,000 m long by 1,571 m around. That is 3.14 km2.
- **The whole drum is one 15-minute district.** A concourse at mid-length is at most 1,271 m from any point, which is 15.1 minutes at 1.4 m/s.
- **Neighbourhood nodes:** one every 500 m along and two around gives 8 nodes. The farthest home is 466 m (5.5 minutes) from its node. That is close to Perry: 3.14 / 0.65 is about 5 Perry units.
- **End to end on foot:** 24 minutes. The farthest pair of points (2,148 m apart) is 26 minutes.
- **Rail:** a line averaging 25 to 35 km/h including stops covers 2 km in 3.4 to 4.8 minutes, plus the wait.
  - Copenhagen's metro "has a scheduled average speed of about 35 km/h, with only 1 km between stations".
  - Paris Line 1, with "an average of 700 m between stations", rose from "24·4 km/h to 30 km/h" when automated ([Metro Report](https://www.railwaygazette.com/45910.article), 2018-02-07).
- **On bigger drums, rail stops being optional.** A 500 m by 8 km drum is 95 minutes on foot end to end, against about 16 minutes by rail.
- **Spoke elevators,** rim to axis, 250 m:
  - 100 seconds at 2.5 m/s, the top of the geared-traction range ([APPA](https://www.appa.org/elevator-systems), no date shown).
  - At 1 g on a 250 m radius, the spin is ω = 0.198 rad/s. A car moving radially at 2.5 m/s feels 0.99 m/s2 of sideways Coriolis push, about 0.1 g. At 10 m/s it would feel 0.4 g. So spoke lifts stay slow, which is a detail for the full-realism mode.

### 2.7 Capacity

Assumption: 25% of the drum goes to streets, civic buildings and parks. Perry's floor is 10% for parks alone. That leaves 2.36 km2.

- **All Homesteads:** 481 plots, housing 481 to 1,443 people.
- **Small homes with shared fields:** food land is the limit. 2.36 km2 divided by 260 to 1,000 m2 per person gives about 2,400 to 9,000 residents. That only works if goods move.
- **The "~780 one-acre allotments"** in habitat-generation.md counts 4,047 m2 acres with no streets. It has to be recomputed for a mix of starts.
- **The mix of starts is a per-ship setting in data.** The game can then show the trade between self-sufficiency and land on one side, and density and logistics on the other.
- **Energy is a second limit** under artificial light: about 4.8 kWh per m2 per day for staple crops (docs/design/self-sufficiency.md:91-96).

### 2.8 Placement

- **Homes go in the drum, at 1 g. None go in the spine.** The spine runs on cruise thrust of 0.05 to 0.1 g (docs/design/gravity-and-movement.md:60-70), which suits hangars, freight, industry and storage, not daily life.
- **Density follows transit.** Transit-oriented development puts "The densest areas ... within a radius of 1/4 to 1/2 mile (400 to 800 m) around the central transit stop" ([Wikipedia](https://en.wikipedia.org/wiki/Transit-oriented_development), fetched 2026-10-03).
  - Cabin blocks sit at the station.
  - Apartment blocks come next.
  - Homesteads form a ring beyond, next to the agriculture zones.
- **Perishables sit nearest the people.** Von Thünen: perishables "must get to market quickly" and so are "produced close to the city" ([Wikipedia](https://en.wikipedia.org/wiki/Von_Th%C3%BCnen_model), edited 2026-03-04).
  - Around each node, from the centre out: mess hall, then greens, herbs and eggs, then homes.
  - Bulk fields, orchards and bulk storage go furthest out.
- **Noise and freight get their own space.**
  - Industry, the reactor, hangars and storage go in the spine, behind a noise buffer. The operator, 2026-07-01: "if the industrial area is too close, no one can sleep" (docs/design/mothership-superstructure.md:47-66).
  - Freight lanes run behind the homes, apart from passenger paths.
  - No single elevator or line failure may isolate a district (docs/game/ship_zoning_transit.md:77-110).
  - Each home or block has a parcel locker on the freight side.
- **New zone types, as data:** `mess_hall`, `recreation`, `cabin_block`, `apartment_block` (with its common house) and `street`.
  - data/blueprints/zone_types.ron:9-93 has none of them at district scale.
  - The `commons` room type already covers "dining, meetings, and recreation" (data/rooms.ron:481-491).

### 2.9 Planning rules as data tests

These keep "it makes logistical sense" true as the ship grows. They are also what makes the ship file usable as a planning model (section 6).

- Every home plot reaches a mess hall within **5 minutes** along the corridor graph.
- Every home plot reaches every daily need (mess hall, clinic, school or Library, recreation, station) within **15 minutes**.
- No home plot lies within **M metres** of an industrial or reactor zone. M is open question 14.
- Freight routes never share passenger corridors.
- No single elevator or line failure isolates a district.

---

## 3. Meeting places and missions

*Taken from:*
- the place table: Proposal 3;
- "draw people with what only a shared place can do" and the free crew meal: Proposal 2;
- the in-person mission departure: all three, with Proposal 2's early-departure rule.

*No contradictions.* Proposal 1 left free meals as part of the open NPC question. They are recommended here as a separate decision (question 11).

**The principle:** each shared place offers something cheaper to share than to own, and runs on a timetable, so people are there at the same time. No stat bonuses.
- Cohousing's paths between homes are "the opportunity for interaction" ([PBS](https://www.pbs.org/newshour/nation/balancing-privacy-community-design-cohousing), 2017-02-18). So paths are routed past each other.
- The ship's places belong to the ship, never to a leader (the WoW garrison lesson in 1.2).

| Place | Level | Why people come | Rhythm |
|---|---|---|---|
| Mess hall (in the common house) | neighbourhood | free metered crew meals (the Cabin's food), better meals from homesteaders' produce, the block's rota kitchen, parcel lockers, a board for courier and crew jobs | meal times |
| Recreation room | neighbourhood | games, music, shared in-world screens for watching together, a small arena | events |
| Clinic, school or Library room | neighbourhood | care, learning, the Real Skills curriculum | opening hours |
| Station | neighbourhood | rail stop and freight depot | departures |
| Concourse (mall-1) | district | the market: stalls with owners and listings, goods drop-off and pick-up | market days |
| Crafting halls | district (spine) | machines too big for any home, a tool library, people to learn from | booking slots |
| Hangar | spine | private ships in bays, mission transports, the freight hub | departure times |

**Details:**
- **The mess hall has several entrances.** The Ford rebuilt its mess decks with a "hub-and-spoke design that provides three entry points" to end the queues (Navy Times, 2014-10-13). rooms.ron's commons equipment already lists a notice board (data/rooms.ron:486).
- **The Concourse.** Move the three `trading_post` machines from the player's data/machines/home.ron into ship data.
- **Screens.** Watching YouTube, Twitch and Rumble on in-world monitors is the operator's stated goal (CLAUDE.md, the 2026-09-18 amendment). The recreation room is where people watch together.
- **Two civic tiers:** the neighbourhood node and the district Concourse. This answers the open question in mothership-superstructure.md:68-86 if the operator agrees (question 13).
- **The NPC crew keeps these places alive.** The mall or hangar is already the first planned crowd test (docs/design/npc-crowd-stress.md:3-15).

**Missions from the hangar.**
- **A departure is a record:** transport, bay, time, seats and mission.
- **Signing up:** from anywhere, through the departure board in the hangar or on your home screen.
- **Boarding is in person.** You must stand inside the bay's boarding zone at departure time. There is no matchmaking teleport.
- **When it leaves:** on schedule, or early once every seat is filled. Deep Rock Galactic launches when the host waits 15 seconds, cut to 5 "if the entire team is inside" ([wiki](https://deeprockgalactic.wiki.gg/wiki/Space_Rig), edited 2026-03-15).
- **Why in person matters.** WoW's lead designer on finding a group in person: "these are real people, these aren't just faceless automatons" ([PCGamesN](https://pcgamesn.com/world-of-warcraft-warlords-of-draenor/warlords-of-draenor-group-finder-will-return-world-of-warcraft-to-being-a-place-where-you-can-actually-meet-new-people), updated 2017-08-17).
- **Depends on the speed check** (increment 4). Without it, a client can simply claim to be standing in the bay.

**Hangar or home bay.** The acre's own vehicle bay keeps ground vehicles and drones, and spacecraft live in the shared hangar (question 15).

---

## 4. Build rights

*Taken from:*
- the plot frame and `may_build`: Proposal 1 and the server survey;
- the permit certificate: Proposal 3 (it also covers Proposal 2's household members);
- the server-granted rank: all three.

*Contradictions resolved:*
- **Proposal 2's household certificate versus Proposal 3's build permit.** These become one certificate. The owner signs `hum/permit/v1\n{plot}\n{grantee}\n{expiry}` with their Dilithium3 key, and the grantee holds it. A household member is simply a permit with no expiry.
- **Who may take pieces down.** Proposal 1 said the owner alone; Proposal 3 said owner and permit holders. Resolved:
  - the owner may remove any piece on their plot;
  - a permit holder may remove only pieces they placed (shared-building.md's stored `owner` field already supports this);
  - visitors may remove nothing.

**The rules:**

1. **Your plot: you build.**
   - Once the districts leave the home body (increment 1a), the B-key editor in normal play edits only your home.
   - Blueprint pieces placed with E go aboard the ship only inside your plot's box. Dev mode is exempt. Planet sites are unaffected.
2. **Household and helpers** build through the permit certificate. The relay checks it without storing it. This is the house pattern: "the certificate pattern (client-held, statelessly verified) is the house answer" (CLAUDE.md, storage schema; the precedent is `verify_friend_cert` in src/relay/core/pq_crypto.rs).
3. **Visitors** never build.
4. **Block shared areas:**
   - premade and kept up by the ship;
   - later, residents furnish marked fit-out areas;
   - later still, changes go to a vote using the signed voting that is already live.
   - Planting the block garden is farming, not construction, and is open to residents from the start.
5. **Ship spaces** need a server-granted rank, `can_edit_ship`, in the relay's roles table. That rank is still the unratified Mode/Rank proposal (game-modes.md:26-76; question 9).
   - Local Dev mode no longer grants ship editing while joined to a shared world.
   - Today it does: Dev is the default (config.rs:130-131).
6. **Today's defect, fixed in increment 1a.** The Zones, Rail and Roads panels edit the home body, which holds the 13 districts. A Normal-mode player can therefore move the hangar (construction.rs:1640-1680, 2321-2350).
7. **How the relay enforces it:** `may_build(did, frame, box)` is true when:
   - the frame is `plot:<id>`;
   - the box lies inside [0, size];
   - the builder owns the plot or holds a valid permit for it.

   The relay can only check pieces it is sent. A purely local build stays unchecked, but it is also invisible to everyone else.
8. **Materials** are trusted until holdings exist (increment 8). After that, the relay charges them.
   - The operator's 2026-10-03 words settle only *where* a player builds, so Blocked item 5 ("does multiplayer enforce anything", docs/PRIORITIES.md:523-525) stays open on materials.
   - The uncommitted PRIORITIES edit that drops "co-op trust or enforced rules" from the Waiting list reads more into his answer than he said.

---

## 5. Goods transport

*Taken from:*
- "one place at a time" and the size classes: Proposal 3;
- the carrier table: all three, with speeds from the world survey;
- "friction in choices, never in menus": all three;
- shipment mechanics and drawing robots locally: Proposal 1;
- the two modes and the worked trade: Proposals 2 and 3.

*Contradictions resolved:*
- **Data speeds versus sources.** data/transportation.ron has no sources, and some of its figures exceed real ones:
  - the AGV is 2.0 m/s (data/transportation.ron:275-279), while real ones run about 1 m/s;
  - the pneumatic tube is 8.0 m/s with 10 kg capsules (:226-229), while real ones run 5.5 to 7.6 m/s with loads up to 5 lb;
  - the rail speed limits (light_rail 22.2 m/s at :93, cargo_rail 27.8 m/s at :127) are unsourced.

  Travel times here use the sourced figures. The data file is corrected through a dated findings document before it sets any time in the game.
- **Rail.** Proposal 2 used cargo_rail's top speed (72 s for 2 km). Proposal 3 used metro averages including stops (3.4 to 4.8 minutes). Planning figures use averages including stops. Line data carries a top speed plus a dwell time per stop, so the average falls out of the data.

### 5.1 The rule

**An item sits in exactly one place:** a locker, someone's hands, a carrier, or a shop. Trades stop teleporting.

What exists today:
- A completed trade only flips a status (src/relay/storage/trading.rs:175-186). Each client then edits its own backpack (src/gui/pages/trade.rs:564-581).
- The header comment at trading.rs:1-3, "Items are locked in DB", is wrong, because the relay holds no items. Correct it.
- `TransportationSystem` exists (src/systems/transportation.rs) but is never constructed.
- The mining drone, with its away-time catch-up (src/systems/mining.rs:358), is the one working carrier with a travel time. It is the precedent for courier robots.

### 5.2 Size classes (a data file)

| Class | Limit | Carried by | Source |
|---|---|---|---|
| Parcel | up to 2.3 kg | tube | carriers "can hold up to five pounds" ([MIT Technology Review](https://www.technologyreview.com/2024/06/19/1093446/pneumatic-tubes-hospitals/), 2024-06-19) |
| Carry | 50 kg, 65 L | the player | src/gui/pages/inventory.rs:38-40 |
| Tote | robot load | courier robots, cart track | |
| Pallet | 800 x 1,200 mm, 1,500 kg | freight lift, freight rail, forklift | [EUR-pallet](https://en.wikipedia.org/wiki/EUR-pallet), 2026-09-28 |
| Container | | hangar, shuttle | |

### 5.3 Carriers, speeds and times

Times are computed for a neighbourhood (400 m) and for the 2 km drum.

| Carrier | Speed | 400 m | 2 km | Source |
|---|---|---|---|---|
| Player on foot | 1.10-1.65 m/s (1.4 design) | 4.0-6.1 min | 20-30 min | [Preferred walking speed](https://en.wikipedia.org/wiki/Preferred_walking_speed), 2026-07-27 |
| Courier robot (AGV) | about 1 m/s; fast systems 3.3 m/s | 6.7 min | 33 min (10 min fast) | "At 60 meters per minute ... you don't implement AGVs for speed" ([MMH](https://www.mmh.com/article/building_the_faster_safer_agv), 2014-05-01) |
| Pneumatic tube, parcels only | 5.5-7.6 m/s | about 1 min | 4.4-6.1 min | MIT TR 2024-06-19; [Stanford Medicine](https://med.stanford.edu/news/all-news/2010/01/gone-with-the-wind-tubes-are-whisking-samples-across-hospital.html), 2010-01-11 |
| Cart track | 10 m/s | 40 s | 3.3 min | Heathrow T5 carts "operating at 10m a second" ([Airport Technology](https://www.airport-technology.com/?p=20120), 2008-03-26) |
| Freight rail | 25-35 km/h average with stops (assumed equal to passenger metro) | | 3.4-4.8 min plus the wait | Metro Report 2018-02-07 |
| Freight lift | 0.76-2.5 m/s | | 40-132 s per 100 m of rise; a 250 m spoke takes 100-330 s | [APPA](https://www.appa.org/elevator-systems), no date; freight lifts carry up to 100,000 lb ([TK Elevator](https://www.tkelevator.com/us-en/company/insights/service-elevator-freight-elevator-differences.html), 2022) |
| Shuttle between ships | orbital mechanics | | tens of minutes and up | nearest real hop: a 43-minute Soyuz port relocation ([Spaceflight Now](https://spaceflightnow.com/2021/09/28/soyuz-ms-18-relocation/), 2021-09-28) |

Handling time per transfer is a per-node value in data. It has no verified source yet: the survey's quay-crane figures came from a search summary and are not used.

### 5.4 A worked trade (computed)

A homestead at one end of the drum sells 20 kg of potatoes to a Cabin resident at the other end:
1. A robot takes the crate from the seller's locker to the neighbourhood depot, 400 m away: 6.7 minutes.
2. Freight rail carries it 1.5 km between depots: 2.6 minutes, plus half the headway. With a 10-minute headway (assumption), that is 5 minutes.
3. It lands in the parcel locker beside the buyer's mess hall, and the buyer collects it on the way to dinner.

The total is about 15 minutes plus handling. Carried by hand it is about 24 to 26 minutes. The tube cannot take it, because 20 kg is far over its 2.3 kg limit.

### 5.5 Who carries

1. **You,** free, within your backpack's mass and volume. Inside a neighbourhood this is the fastest choice.
2. **The ship's freight service,** for a small fee: tube for parcels, cart track for totes, rail and lifts for pallets.
3. **Your own courier robot:** slow, but free once built. On the relay it runs the existing crew chore cycle (assign, travel, work, done; src/relay/handlers/game_state.rs:142-176), carrying a shipment id instead of a chore.
4. **Another player,** on a courier contract with collateral.
   - The package is sealed: the courier carries it but cannot use it.
   - In EVE the game holds the package in escrow, and the hauler's collateral is forfeited on failure ([EVE University](https://wiki.eveuniversity.org/Contracts), edited 2023-07-19).
   - Hauling becomes a real profession: Red Frog handles "around 600-700 contracts daily" ([EVE Online](https://eveonline.com/article/community-spotlight-red-frog-freight), 2013-06-21).

**Carry it yourself, or pay for slower automatic handling.** Star Citizen 3.24 made the same choice: hand loading, or loading "at an aUEC cost with reduced speed" ([starcitizen.tools](https://starcitizen.tools/Update%3AStar_Citizen_Alpha_3.24.0), released 2024-08-29).

### 5.6 Keeping it interesting rather than tedious

- **Goods travel without you.** Sending is one action that shows the arrival time. Deposits and withdrawals at a locker are instant. Parcels arrive while you are offline.
  - Foxhole's 49-day logistics strike ended with "quicker pull times" and changes to make logistics "less cumbersome" ([NME](https://www.nme.com/news/foxhole-logistics-union-ends-49-day-strike-after-demands-met-3173270), 2022-03-02).
  - Players accept distance. They revolt against waiting in menus.
- **Route, arrival time and price are shown up front.** The player picks cheap and slow, or fast and paid.
  - "Each day in transit is equivalent to an ad-valorem tariff of 0.6 to 2.1 percent" (Hummels and Schaur, [AER 103(7)](https://ideas.repec.org/a/aea/aecrev/v103y2013i7p2935-59.html), December 2013). That supports an express tier, especially for perishables and between ships.
  - The last mile is "41% of total logistics supply chain costs" ([Capgemini](https://www.capgemini.com/insights/expert-perspectives/navigating-the-complex-web-of-last-mile-deliveries/), 2023-10-05). Shared parcel lockers at the mess hall are the real-world fix, and they give people a reason to visit.
- **Riding should be pleasant.** Trains get windows onto the drum and other players aboard, so a trip is a short scene, not a loading screen.
- **Distance makes places matter.** Produce is cheaper near where it grows, so hauling between neighbourhoods is worth doing.
- **Food keeps flowing whatever players do.**
  - When EVE slowed long jumps, freighters got a 90% exemption "to ease the impact of these changes on alliance logistics" ([EVE Online](https://www.eveonline.com/news/view/phoebe-travel-change-update), 2014-10-30).
  - Here, NPC freight runs as aggregate flows per route and keeps every mess hall stocked.

### 5.7 Two modes (the CLAUDE.md house rule)

- **Simplified:** one button, and the ship's service picks the route and carrier. Fixed time per hop from the lines data, unlimited capacity, nothing lost. Travel time is still real, so trade stays fair.
- **Full realism:** capacity per departure, handling time, robot energy, outages with fallback routes, spoilage in transit, and sealed packages with collateral.

Both modes write the same shipment record. Only the arrival-time maths and the failure rules differ.

### 5.8 Teleporters

- If teleporters carry goods, travel time disappears. So: **never goods.**
- **People:** developer or rank only for now, as in the lore (docs/game/humanity_one.md:56-61).
- Question 10 asks whether players later get them between transit hubs at an energy cost.

### 5.9 Between motherships

- **Same relay:**
  - a `world_id` on plots, holdings and shipments, and one frame per ship;
  - a shuttle line with real travel time (no FTL).
  - Ships in formation: hangar handling, a shuttle hop of tens of minutes, and the far ship's internal leg, about an hour in all (assumption until fleet geometry exists).
  - Elsewhere in the system: hours, set by orbital mechanics. The travel table in gravity-and-movement.md:322-344 still uses the retired 72x clock and must be redone.
- **Different relays:**
  - the origin server moves the goods into `shipment:<id>` and sends a `cargo_manifest_v1` object signed with its Dilithium key over the existing federation;
  - federation already drops duplicate object ids (src/relay/handlers/federation.rs:720), which stops a manifest being replayed;
  - manifests are accepted only from peers the server admin has pinned, because a federated server could otherwise create goods from nothing (question 16).

### 5.10 What the server holds

**Three fixes come first:**
1. **Replace the 100 m rule** with a speed check on the sender's own clock: distance at most a maximum speed times the time between updates, plus slack.
   - A refused update sends the sender a correction instead of freezing their stored position.
   - Registered transit jumps (lifts, rail, rank teleporters) carry a link id that the relay checks against the ship file.
   - The rule today has no time component. It still lets a modified client claim about 1.5 km/s by staying under 100 m per update at 15 Hz (msg_handlers.rs:3285-3296).
2. **Deliver game events by zone,** or to the people involved, not to every socket (src/relay/relay.rs:28-62). Positions alone are about 45 KB/s per socket at 12 players (docs/design/shared-building.md section 5).
3. **The relay's world becomes this ship** instead of the six-room Pioneer (game_state.rs:313-418).

**New tables.** Each is new and gets its own batch, so the BUG-046 rule is met trivially.

| Table | Holds | Lifetime |
|---|---|---|
| `game_plots (world_id, plot_id, owner_did, assigned_at)`, primary key (world_id, plot_id), UNIQUE (world_id, owner_did) | ownership only; geometry stays in data | while owned |
| `world_pieces` (shared-building.md) | built pieces, frame `plot:<id>` | permanent |
| `holdings (holder, item_id, wear, quality, qty)` | holders `locker:<plot>`, `carried:<did>`, `trade:<id>`, `shipment:<id>`, `stock:<zone>`. Every move is one transaction that fails if any line is short (the `fill_trade_order` pattern, trading.rs:324-391) | permanent |
| `shipments (id, world_from, world_to, to_holder, carrier, depart_ms, arrive_ms, status)`, indexed on (status, arrive_ms) | goods in flight | deleted on delivery |
| `departures`, `seats` | mission transports | until departure |

**Kept as data, not tables:**
- starts (`data/homes/starts.ron`);
- each ship's plot layout;
- carriers;
- the freight and passenger graph;
- lines and timetables, shaped after GTFS: stops, routes, trips, stop_times, and frequencies for "Headway (time between trips)" ([gtfs.org](https://gtfs.org/documentation/schedule/reference/), revised 2026-04-27).

The next departure is computed from a start time plus the headway, never stored.

**Delivery and the clock:**
- A sweep once a second, inside the existing 50 ms loop (src/relay/mod.rs:645-677), delivers due shipments in one transaction and tells only the people involved.
- Arrivals run on the server's wall clock, never on `game_time`, which restarts when the snapshot key changes (game_state.rs:319).
- Clients already receive `server_time` every 5 s (relay/mod.rs:762-793), so "next train in 2:13" needs no new message.

**Never held by the server:**
- cosmetic simulation: robot and cart motion (each client interpolates along the route from depart and arrive times), scaffold growth;
- the private save;
- a home's full layout, until the owner opens it to visitors. home.ron alone is 172 KB, over the relay's 128 KB message cap (relay/mod.rs:1244), so it would travel in parts.

**Privacy:**
- `owner_did` is used only for build and delivery checks. No endpoint lists who lives where, and door nameplates are opt-in and sent by the client.
- Block membership is derived from location, never stored as a list.
- Shipments keep no sender column and are deleted on delivery, the sealed-sender mailbox pattern, so the relay never builds a record of who trades with whom.
- Completed rows in `trades` should be purged the same way, since today they keep both parties' keys.

**Settings.** Plot counts, the waiting list, inactivity rules, the moving cooldown and express fees are Server Settings rows (the GUI-first rule).

---

## 6. How this serves city planning

*Taken from:* Proposal 3 section 7, with Proposal 1's planning rules as data tests.

**The same model works in real units:**
- plots are parcels;
- starts are housing types, and their space minimums are validation rules (the UK standard's figures make a ruleset);
- zones map to OpenStreetMap land-use values such as "residential", "retail", "industrial", "farmland" and "allotments" ([OSM wiki](https://wiki.openstreetmap.org/wiki/Key:landuse), edited 2026-05-11);
- lines shaped like GTFS mean a real city's transit feed can be loaded.

**The outputs are cheap graph queries, not simulation:**
- **Walk-time coverage:** the share of homes within 5 and 15 minutes of a mess hall, clinic, school or station. Run it at several speeds (1.10, 1.33 and 1.4 m/s, and slower). That answers the criticism that 15-minute maps misjudge how far elderly people can get.
- **Food land per resident** against the farmland available.
- **Freight arrival times and vehicle-kilometres.**
- **Capacity for each mix of housing types,** as in 2.7.
- **The planning rules of 2.9** reported as pass or fail per plot.

**Where it shows up:** the Real side of "two realities" (docs/design/two-realities.md:7-36), as a "plan a neighbourhood" page running on the same engine. Label it a sketching and teaching tool, not professional planning advice.

---

## 7. Increments, in build order

*Taken from:* Proposal 1's order and proofs (the only one with a rig check for each step), with Proposal 2's mess hall moved next to the Cabin and Proposal 3's height gate before any stacked home.

*Contradictions resolved:*
- **When the relay stops simulating the Pioneer.** Proposal 3 makes it a prerequisite of plots; Proposal 1 does it after the target. **After.**
  - Increment 1 needs the relay only to spawn players at their plots.
  - The Pioneer's rooms keep driving the starter quest until increment 3. A spawn outside every Pioneer room just records "quarters" as the first room visited (game_state.rs:591-593), which is odd but harmless for one increment.
- **When the speed check lands.** Proposal 3 makes it a prerequisite; Proposal 1 puts it after the target. **After.**
  - Spawning at the plot keeps walking under 100 m per update, and the rig respects that.
  - Anything faster (transit, carried goods, boarding) waits for increment 4.

Each increment is proven on the state the player reaches, and its failing case must be seen before its passing case is trusted.

### Increment 1, this week: two players, two homes

It comes in two halves, and each one ships.

**1a. The ship and the home come apart** (data and client; ships alone).

What changes:
- **One rewrite, by script, with no compatibility path** (the pre-launch rule):
  - the ship file keeps the Commons, the 13 districts (moved up to ship level), street-1, the corridors and a `plots` list (p1, p2);
  - the home's 55 x 89 m body moves to its own design file.
- **The loader assembles the ship:** the ship file, plus "my home" as zone id `home` at my plot's origin, plus my plot's door corridor.
  - Offline play uses the ship's default plot, p1.
  - `home` becomes a local alias for the viewer's own home, so the eleven call sites that assume it, the machines' default zone and `PLAYER_HOME` keep working unchanged.
- **Machine offsets become zone-local.**
  - The home rows do not change numerically, because p1 is at the origin.
  - The 11 Commons rows move to a ship machine file, made Commons-local by subtracting (65, 20).
- **Spawn:** the world-load spawn adds the zone origin.
- **Overlap:** `validate()` rejects overlapping plots, zones and corridor tubes.
- **Clones:** the `res-1` clone tiling is switched off, because it overlaps the Commons. Increment 2 draws one shell per real plot instead.
- **Saving:** the home file is written always, and the ship file only with ShipStructureEditing. The home's origin is never saved, because it comes from the plot. This also fixes the Normal-mode hangar-moving defect.
- **E-key pieces** go aboard only inside your own plot box (Dev exempt).

Files:
- `data/blueprints/ship_structure.ron` (rewritten);
- NEW `data/homes/homestead.ron` (body, spawn 53.5/40.5, door at local z 40);
- NEW `data/machines/ship.ron`;
- `data/machines/home.ron` (Commons rows removed);
- `src/ship/ship_structure.rs` (the `Plot` struct, `plots`, overlap rules in `validate`, `assemble`, tests);
- `src/ship/home_structure.rs` (clone tiling off, 1004-1100);
- `src/machines.rs` (zone-local resolve at 553-561, ship machine file);
- `src/engine/world_load.rs` (load and assemble, spawn at 199-202);
- `src/engine/editor.rs` (263-268) and `src/lib.rs` (7279-7292) for saving;
- `src/gui/pages/construction.rs` (1640-1680, 2321-2350);
- `src/engine/build_place.rs` (E-key plot bound);
- every other reader of ship_structure.ron, which must go through `assemble`. grep finds `src/ship/hull.rs`, `src/ship/room_types.rs`, `src/engine/room_gi.rs`, `src/gui/mod.rs`, `src/systems/farming/humidity_tests.rs`, `src/systems/farming/life_support_tests.rs`, `scripts/photograph-home.js` and `scripts/home-vantages.json`;
- NEW `scripts/split-ship-home.js`, run once and deleted in the same commit.

Proof:
- **Lib tests:**
  - assembling at p1 reproduces today's room count and wall-segment count, pinned from numbers taken before the split;
  - moving the plot by (dx, 0, dz) moves every room box, collision segment, machine and the spawn by exactly (dx, 0, dz). Today this fails on the spawn and the machines, and that failure is the case to see first;
  - an overlap table test.
- **Unchanged rigs:** `just verify-copresence` and `just verify-screens` pass as they are, because the rig's pose (30, 1.7, 20) is still inside p1 (scripts/verify-copresence.js:92).
- `just validate-data`.

**1b. The relay hands out plots** (relay and client; one release).

The two halves must land together. A relay that spawns a player at p2 while that client still draws its home at p1 freezes the player under the 100 m rule.

What changes:
- **The relay loads the ship's plots** through the `ship` module. It has an embedded fallback, because the throwaway relay runs in a folder with no data (scripts/lib/throwaway-relay.js:209).
- **The table:** a new `game_plots` table.
- **On a fresh `game_join`:**
  - find the player's plot by DID; otherwise claim the first free plot. The unique constraint means two simultaneous joins cannot share a plot;
  - spawn at the plot's origin plus the design's spawn point;
  - `game_welcome` gains `home_plot {id, kind, origin, size}` and `ship {id, hash}`;
  - if the ship is full, `home_plot` is null and the player spawns in the Commons as a guest.
- **The client applies the welcome:** it moves its `home` zone to `home_plot.origin`, attaches the plot's corridor, rebuilds through the existing dirty path, and puts the camera at the plot's spawn.
- **Ship hash:** on a mismatch the client refuses to join, with one plain sentence: positions only agree when everyone has the same ship.

Files:
- NEW `src/relay/storage/plots.rs`;
- `src/relay/storage/mod.rs` (module and table);
- `src/relay/handlers/game_state.rs` (plots on GameWorld, spawn at 581-589);
- `src/relay/handlers/msg_handlers.rs` (join at 2905-3091, welcome at 3085-3091);
- `src/relay/features.rs` (tests);
- `src/net/protocol.rs` and `src/engine/net_route.rs` (the welcome arm at :32);
- NEW `src/engine/home_plot.rs` (the pure apply function and its test);
- `src/engine/ipc.rs` (the recorder's one-frame probe reports the viewer's plot);
- `scripts/second-player.js` and `scripts/tests/second-player.test.js` (log `home_plot`, start paths at your own plot's spawn);
- `scripts/verify-copresence.js` (a `--plots` mode that runs both join orders);
- `scripts/lib/copresence-judge.js` and `scripts/tests/copresence-judge.test.js`;
- `Justfile`.

Proof:
- **Real-relay tests in features.rs:**
  - two keys get different plots;
  - the same key keeps its plot across a socket close and across a relay restart;
  - swapping the join order swaps the assignments, so the first joiner gets no privilege;
  - the spawn equals the plot's spawn;
  - a full ship gives null.
- **The red case first:** on today's code, a player on p2 has their first position update refused (138.7 m from the Pioneer spawn).
- **The rig, `just verify-copresence --plots`, run twice:**
  - Walker first: the walker gets p1 and the game gets p2.
  - Game first: the game gets p1 and the walker gets p2.
  - In each run the judge checks:
    - the two `home_plot` ids differ;
    - after joining, the game's camera is inside its own plot's box;
    - every position the game *drew* for the walker (from the recorder, which records off-screen figures too) lies inside the walker's plot;
    - the existing smoothness checks pass on those samples.
  - The in-view and screenshot checks wait for increment 2, because the two players cannot see each other yet.
  - With 1b reverted, "camera inside p2" fails in the walker-first run.

**1b as built (2026-10-03), where it differs from the plan above:**
- **The ship got an id.** The ship file had none, so `id: "mothership-1"` was added; it is the `world_id` of `game_plots`. The hash is 16 hex digits of FNV-1a over the PARSED ship file (`ShipStructure::ship_hash`), so comments and number formatting do not change it, and an assembled ship hashes the same as its file.
- **The player arrives at their OWN door, and the game stands where the relay holds it.** (Corrected after the first review: the first build claimed the relay's spawn and the game's camera "cannot drift" because both used `ShipStructure::plot_spawn`. They could: the relay used its own copy of the default design, and the player can move their door with the build-mode avatar, up to 72 m away on this ship.) Now:
  - the game's `game_join` carries `home_spawn`, its own home's door in plot-local metres (`ShipStructure::home_arrival_local`), and the relay spawns the player there on whichever plot it hands out, kept inside the plot (`PlotArrival::arrival`);
  - a home with NO authored door names none, and the relay and the game both take the middle of the plot actually handed out (the second review: the game used to send the middle of the plot it was built on, which matched the relay only while every plot was the same size, and planned apartment plots will not be). A scripted player, which draws no home, also arrives in the middle of its plot;
  - every welcome says whether the relay found the player still in the world (`rejoin`: true for a reconnect inside the 90 s grace, false for a fresh spawn). The game stands the player where the relay holds them (their own entry in the welcome's `world_snapshot`) when `stand_where_held` says so: on an ARRIVAL (the first welcome since the world loaded, or one from another server), on ANY fresh spawn, and whenever the game stands more than 90 m from that point (`FAR_FROM_HELD_M`, inside the relay's 100 m rule). Otherwise, a reconnect near where the relay kept them, the player keeps walking where they are. (The second review found the first rule, "only on an arrival", froze every player who stepped out to the launcher's offline home, Dev travel or fly mode and back, outlasted the grace, or met a relay restart: the relay respawned them at their door or in the Commons while the game left them standing up to 148 m away, and every update was refused);
  - the first build sent a reconnecting guest back to the Commons, 148 m from where the relay held them at the end of street-1: a reconnect inside the grace keeps walking where it is.
- **Only a join naming this relay's ship holds a plot.** The `game_join` carries `ship_hash` (empty when no ship assembled). A join naming ANOTHER ship is refused at the join, before anything is spawned or broadcast (`game_join_denied`, reason `other_ship`, with the one plain sentence and the relay's ship): the first build spawned it, welcomed it, and everyone saw a player join and leave at once. A join naming NO ship (an AI agent, a test bot that did not ask) is a guest in the Commons: the first build claimed a plot for it, held for good, so a few test bots could fill the ship. The ship `{id, hash}` is on the public `/api/server-info` (it was already in every welcome), and the rig's walker (`scripts/second-player.js`) and `scripts/ai-sample-client.js` read it there and name it.
- **A plot can be given back.**
  - An admin releases a plot from Server Settings > ADMIN > Homes on the ship (native, `src/gui/pages/game_admin.rs`; the web chat's Game Admin window mirrors it): `game_release_plot` naming the plot's id (`p1`) or the holder's public key (`src/relay/handlers/home_plots.rs` `handle_game_release_plot`), refused while the holder is in the world (their home stands on the plot, and the next joiner would build on it too). The check is by the id the plot is held under, the same one the release deletes by (`plot_holder_in_world`): the third review found a key pasted in upper case got past a check that compared keys as text. A plot id is the one name an admin always has: nothing lists who lives where, so once a holder's key is gone (an erased account) no key can be typed. The admin action registry (`data/admin/ops_registry.json`) lists it, and a test now checks every code pointer in that registry names a function its file defines (the entry pointed at msg_handlers.rs).
  - Erasing an account gives its plot back, and the account's export lists it first (`storage/account.rs`, `ship_plots`; the third review: an erased account held its plot for good, so two erasures filled the shipped two-plot ship).
  - A game whose home cannot stand on the plot it was given ("does not fit") refuses to join and leaves with `give_up_plot`, so the plot goes back for the next player instead of being held by someone who never lives there.
  - Nothing releases an idle plot by itself: when one should go back is open question 19.
- **A relay whose ship file does not load keeps the ship built into it** (`ShipPlots::load`), the one the same version of the game draws. With no ship at all it refuses a game's join with its own reason (`no_ship`, `NO_SHIP_SENTENCE`): the third review found it told every game "a different ship from yours".
- **A refusal says what to do and stays on screen.** The other-ship sentence ends with "update whichever of the app and the server is older and reconnect", and the HUD shows "Not in the shared world" with the sentence for as long as the refusal holds (`copresence_refused_note`). It holds until a fresh connection to that server, a switch away and back, or a fresh world load (`follow_server`), not, as the comments said, "until the world loads afresh", which only ever happened at app start.
- **What the home holds goes with it.** The rebuild redoes walls, collision, lights, machines and doors. The move also carries the farm animals and decoration plants (each by the shift of the machine it was placed around, `home_plot.rs` `machine_shifts`, so they keep their health, yield timers and deaths), the pieces the player built aboard and their parked vehicles (`carry_built_pieces`: anything without a planet site standing over the old plot, kept with its uid, so a chest keeps its contents; the first two builds left them on the old plot, by then someone else's home), the hologram, the showroom stage, and the Respawn point (`fps_spawn`). On the player's own plot the Respawn point is their door there, and a guest's is the Commons (the second review: the Stay case left it wherever another server's welcome had put it). The recorder reports all of these (`home_things`, now with `structures` and `vehicles`) and `--plots` judges each against the game's plot.
  - Since the third review the pieces, vehicles, hologram and stage go with the home inside the rebuild itself (`home_meshes.rs` `rebuild_homestead` calls `home_plot.rs` `follow_home_box`): whenever the plot the home stands on has moved since the last rebuild, whatever moved it (a welcome, the Dev Plots panel's `move_plot`, an undo of either), they are carried by the move and the save's frame is republished. A resize or a new default plot only republishes the frame.
  - A welcome that moves the home starts the editor's undo history again from the moved home (`history_after_move`): with the editor open across the move, one edit and one Ctrl+Z used to put the home back on its old plot, someone else's, 99 m from everything the move had carried.
  - A player whose plot was released while they were out, and who comes back as a guest, has their home put back on the default plot (`WelcomeHome::Guest` `home_back`); it used to stay on the plot its new holder lives on.
- **The save records where its home stood.** Built pieces and vehicles are saved where they stand, beside the box of the plot the home stood on (`WorldSave::home_plot_box`, from the `HomeFrame` kept with the live ship, or before the world loads the box of the save applied at startup, `save_load::frame_for_save`). A load carries the ones that stood in that box to wherever the home stands then (`carry_saved_pieces`): the world load for the save applied at startup (`carry_loaded_save_home`), the launcher's character pick and a restored snapshot for a save loaded into a running world (`carry_saved_pieces_home`; a save with no box is read as the default plot's, `saved_or_default_box`). A vehicle the save shows standing OUTSIDE the home, on the plot a boot then builds the home on, is marked (`NotTheHomes`) so the welcome's move does not take it into the home, and the mark is saved with it (`outside_home`) until a move of the home has done its work. (The second review's frame, "as if the home stood on the default plot", broke when a Dev made another plot the default, stranding every piece 99 m away on the next launch, and would have broken with increment 2's remembered plot.)
- **Round 4 of the review (2026-10-03)** found eleven more; each was seen red first (the failure text is in each test's comment):
  - **The first world entry no longer joins before the ship exists.** A returning player's game identifies on the main menu, and lib.rs runs the co-presence block before `load_world` in the frame Enter World or Play is pressed, so the join went out with no ship (`ship_hash: ""`), the relay refused it as another ship, and the refusal landed after the world load had cleared refusals: "Not in the shared world" and the false "update the app" sentence until a reconnect. The join gate (`home_plot.rs` `join_step`) now waits for the world and its ship; a world that loaded on the legacy layout says its own ship did not load instead of joining; `add_join_fields` refuses to build a join with no ship; and the relay refuses an empty `ship_hash` with its own reason (`no_ship_named`, `OWN_SHIP_SENTENCE`) on any relay (on a relay with no ship, "" equalled the relay's empty hash and was taken as this ship's). The rig did not see it because the autopilot creates the identity as it enters, so its socket identifies after the world loads; `--plots` game-first now comes in the returning player's way (`--entry menu`: the autopilot's `"enter": false`, wait for the handshake, answer the privacy window, press the menu's own Enter World), and the judge's `entered_from_menu_after_identify` fails a run on which the race did not really run. Seen red on this branch before the fix (296d70679 with the menu entry added to the rig), run 20261004-040343-plots-game-first: `game_in_world` failed with `refused=true` and the other-ship sentence, the relay log `join refused (other_ship, theirs Some(""))`.
  - **Standing the player where the relay holds them ends a drive and a follow** (`stand_player_at`, `step_out_of_vehicles`, which Respawn uses too): the cab and the follow cam put the camera back on their vehicle every frame, so a welcome's move was undone the next frame and every update was refused.
  - **A welcome while the character showroom is open** moves the point the showroom closes onto, instead of the showroom's camera, so closing it no longer undoes the welcome.
  - **Erasing an account in the world** takes the figure out and frees the plot in one step, under the world's write lock that every join holds while it claims a plot; the next joiner was handed the plot while the holder's figure still stood on it. Their progress in the shared world (`player_progress`) is now exported and erased too, and taking them out does not write it back. The three lists of what an erase removes (Settings, the web chat, `docs/reference/retention_and_deletion_semantics.md`) name the plot and the progress.
  - **A save written before the world loads** (on quit from the menu, or the periodic save) records the box of the save applied at startup, which its pieces still stand in (`save_load::frame_for_save`); it recorded none. On the legacy layout that box is kept for the session (`carry_loaded_box`), so its saves record it too.
  - **A save that records no box** (written before 1b, or a snapshot from then), loaded into a running world, is read as standing on the live ship's default plot (`saved_or_default_box`), where every home stood then: loaded while the home stood on p2, its pieces were not carried.
  - **The `NotTheHomes` mark is saved** with what it marks (`outside_home` on vehicles and built pieces), so a save written before the welcome no longer adopts a truck left on the boot plot into the home on the next launch.
  - **One refusal sentence per cause, each with a next step:** the game's own ship not loading, a welcome with no plot id, a plot our ship does not have and a plot our home does not fit each have their own (the first two read "a different ship from yours"; the last two named no next step, and the assembly error, which can be a sentence of its own, now goes to the log).
- **Round 5 of the review (2026-10-04)** found six more, each confirmed by two skeptics; each was seen red first (the failure text is in each test's comment):
  - **Shutting the build editor no longer jumps the player out of the relay's reach.** B works anywhere aboard, and shutting the editor stood the player at their build spot (the build-mode avatar, else the home's spawn): from the far end of First Street that was about 150 m, so every update after it was refused and the others saw the figure frozen on the street. In the shared world a build spot more than 90 m (`FAR_FROM_HELD_M`) from where the relay holds the player now leaves them where they stood, with a notice saying why (`home_plot.rs` `editor_close_spot`, `EDITOR_HELD_BACK`). Stepping out and joining again, as Respawn does, was the other option, and it was not taken: a fresh join stands the player at their door, which is neither where they stood nor where the avatar stands, and the others would see the figure leave and arrive. A welcome that lands while the editor is open moves the point the editor closes onto and leaves its orbit camera alone, as for the showroom (`stand_player_at`). `--plots` now opens and shuts the editor at the door (the showcase `build_editor` verb, which runs the B key's own function, `editor.rs` `toggle_build_editor`), walks to the far place and opens and shuts it again; `judgeEditorClose` checks the game still stands where the relay holds it and its next move reaches the walker. Seen red 2026-10-04 on a build of this branch with `editor_close_spot` and `stand_player_at` carrying the c8b3a8d54 rule, run 20261004-053342-plots-game-first: `editor_stands_where_held` failed with the camera 154.38 m from where the relay held the game, and `editor_moves_reach_others` with no update seen (the relay log shows 43 refused updates at dist=153.4 m). On the build of this branch merged with main, `--plots` passes 30/30 in both orders, by autopilot and by the menu entry, and the default rig 21/21; the leg nudges 1 m back along its walk, because the far place can stand at the end of street-1, where a nudge along +Z stops at the end wall.
  - **An erase in the world tells the erasing game it left.** The relay sends it `game_join_denied` with reason `account_erased` and one sentence (`ERASED_SENTENCE`); the game leaves the shared world on its side, shows the sentence under the HUD, and does not join that server again until a fresh connection (`join_denied_sentence`). It used to read the game_player_left everyone gets as somebody else leaving and go on showing the shared world, its updates dropped, and Respawn joined it again, claiming a new plot for the account just erased.
  - **An erase rewrites the stored world in the same step** (`leave_world_for_erase` calls `GameWorld::save_to_db`), so a crash before the next 30 s save no longer restores the erased figure, whose ghost reap wrote the erased account's progress back for good.
  - **Any welcome settles what stands in the home** (`welcome_settles_the_home`): a stay on the plot the home already stands on, or a guest's, clears every `NotTheHomes` mark, not only a move. Since round 4 the mark is saved, so a truck marked at one boot and then standing in a home the next server let it keep stayed marked through every launch and was left behind by the next move.
  - **A save loaded on the legacy layout is the box later saves record** (`carry_loaded_save`): with no home to carry to, the box waiting in the DataStore becomes the loaded save's (or none), so after a snapshot restored there, the saves no longer record the startup save's box while the restored pieces stand in their own.
- **The rebuild is called directly, not through the dirty flag.** The dirty path arms the autosave and checkpoints the undo history; a plot assignment is not an edit (src/engine/home_plot.rs).
- **The shared world is joined only from aboard, and a late welcome is dropped.** The join waits until the player is aboard (`aboard`: not on a Dev trip, not held in a planet's frame), and a welcome is applied only while the game is still joined, not solo, and aboard (`accept_welcome`). The third review: a reconnect while Dev-travelling joined from a planet and the welcome moved the player by ship coordinates in the planet's frame; a welcome landing one round trip after the game stepped out yanked the player to their door.
- **Respawn goes through the relay.** The relay still holds a dead player where they died, up to 154 m from their door, so in the shared world the Respawn button steps out and joins again (`respawn_through_relay`): the relay spawns them afresh at their door and the welcome stands them there. The others see the figure leave and arrive at the door.
- **Switching servers leaves the one the game joined.** A click on a saved server swaps in its live background connection within one frame, so the game never saw a disconnect: it never left the first server (whose relay held its figure frozen for good) and never joined the second, while its updates went to a relay that held nothing for it. `follow_server` notices the switch, sends `game_leave` on the first server's parked connection and forgets the shared world, so the join gate joins the second, whose welcome is an arrival.
- **Nothing is sent before the welcome is applied** (not in the plan). p1's spawn is 99.0 m from p2's, under the 100 m rule, so an update sent from the default plot before the welcome arrived would be ACCEPTED and show the player inside p1, someone else's home, for a moment. `game_welcomed` gates the sending.
- **`src/net/protocol.rs` is untouched.** The welcome is read as JSON in `net_route.rs`; no NetMessage field was needed.
- **A non-hex key** (a server bot's `bot_` key) holds its plot under `key:<key>`, which can never collide with a `did:hum:`.
- **The rig's walker walks back and forth for the whole run** (in walker-first order it joins before the game even boots), so the judge picks ONE forward leg by the clock (`forwardLegStart`, `fromEpochMs`) and the in-view check is switched off (`checkView: false`). The plot each side should hold comes from the join order alone, so a build that hands out no plots fails "camera inside p2" with where the camera really was.
- **`--plots` also steps out of the shared world and back** in each order (the showcase `solo` verb, the switch the launcher's offline home and Dev travel flip): out of the world the game is moved to the place on the ship farthest from its door (127 to 154 m), then rejoins; the relay respawns it at its door, and `judgeRejoin` checks it stands there and that its next move reaches the walker through the relay (the walker logs every other player's join and relayed moves). Seen red 2026-10-03 on an exe with the 65b3e2c0c rule (stand only on an arrival): walker-first failed `rejoin_stands_where_held` with the camera 126.58 m from where the relay held it, and `rejoin_moves_reach_others` (the relay log shows 58 refused updates at dist=126.58); the same rig passes 23/23 in both orders on this build, and its walker-first run also carried the sandbox's parked vehicle from (14, 0, 10) to (14, 0, 109).
- **`--plots` then walks away and presses Respawn** (the third review): back in the world, the game walks to the same far place in steps of at most 40 m (`respawnRoute`, each one the relay accepts), the walker sees the relay hold it there, and the showcase `respawn` verb presses the death screen's button. `judgeRejoin` with the `respawn` prefix checks the game then stands where the relay respawned it and that its next move reaches the walker. Seen red 2026-10-03 on a build of the merged tree with `respawn_through_relay` doing nothing: walker-first failed all three `respawn_*` checks (the walker never saw the game join again, and the relay log shows 243 refused updates at dist=127.5 m from where it still held the game); the same rig passes 26/26 in both orders on the real build, and the default rig 21/21.
- **The default rig mode changed one check.** Its walker now joins on p2 and walks to the line in front of the camera from there, so "approach from behind the line's start" became "the straight approach never runs along the line" (`approachClear`).
- **The default rig waits for the welcome before posing the camera** (`welcomed`), because an arrival's welcome stands the player at their door and would undo a pose set before it.
- **Left for later:**
  - a guest keeps drawing its own home on the default plot (someone else's); *done in increment 2: the home is put away*;
  - every world entry builds the home on the default plot first and the welcome moves it, until increment 2 remembers the plot (the save no longer depends on it: it records where its home stood); *done in increment 2: the remembered plot*;
  - a Dev edit that moves the plot the home stands on (the ship editor's `move_plot`) carries the built pieces, vehicles, hologram and stage, but not the farm animals and decoration plants, which follow their machines only on a welcome's move (the machine shifts are taken around that rebuild); Dev-only, for increment 5's build rights;
  - plots are released by an admin, by a home that does not fit, or by erasing the account, never by time: see open question 19.

### Increment 2: meet in the Commons

What changes:
- **Door points.** A debug request has the game report every corridor's door points, computed by the Rust corridor geometry, so the rig never re-implements corridor maths in JavaScript.
- **The walker** goes from its door through its corridor into the Commons.
- **The game** is moved in steps of 90 m or less from its door into the Commons.
- **Back in force:** the in-view and screenshot checks, with the line inside the Commons, in both join orders.
- **Neighbours.** Other plots are drawn as their kind's default-design shells: render only, no machines, no collision. This replaces `tile_home_clones`.
- **Remembered plot.** The client remembers its plot per server, so the next boot builds the home in the right place before it joins.

Files:
- `src/engine/ipc.rs`;
- `src/ship/ship_structure.rs`;
- `src/ship/home_structure.rs`;
- `src/config.rs`;
- `scripts/verify-copresence.js`;
- `scripts/second-player.js`;
- `scripts/lib/copresence-judge.js` and its test.

This is what the week plan's Day 5 two-person session needs.

**Increment 2 as built (2026-10-04), where it differs from the plan above:**
- **Door points are a request of their own, with the places.** `debug/door_points_request.json` (engine/ipc.rs `door_points_json`, src/ship/door_points.rs) answers with every shared zone and every plot as a PLACE (a plot's door: where its holder arrives, at eye height and in plot-local metres) and every corridor as a DOOR (its two mouths and a STEP a metre inside each end, at eye height). The rig's router (scripts/lib/copresence-judge.js `doorRoute`) goes breadth first over places through doors and crosses each door step to step; nothing of the corridor rules lives in JavaScript. It also replaced the 1b rig's hard-coded walk (`respawnRoute`, the junction at (70, 80) and the corridor x and z), and the far places of the 1b legs are now the shared zones' corners read off the report (`farPlaces`). A Rust test keeps the rig's own copy of the report (scripts/tests/fixtures/door-points-p1.json) equal to what the game reports.
- **The walker names its door, and walks a route.** second-player.js gained `--home-spawn x,z` (its join names a door, as a desktop player's does, so the relay spawns it there instead of in the middle of its plot), `--route` (points walked through in order before its path) and `--route-speed` (3 m/s in the rig), and stops cleanly on a "stop" line on its input: Windows gives a child process no signal it can catch, and a kill leaves the figure in the relay's 90 s grace. In the rig the walker of the 1b legs steps out once they are judged and comes back at its own door for the meeting (the same identity keeps its plot).
- **The meeting.** The game is moved from its door, through its door corridor (and First Street, from p2), to the clear south strip of the Commons in steps of at most 40 m, and stands at (76, 1.7, 64) facing south; the walker walks out through its own corridor into the Commons and along (72..80, 70), 6 m in front. Judged under `meet_*`: every step one the relay accepts and the relay holding the game in the Commons (the walker at home saw its moves), the walk's smoothness and timing, the VIEW, both screenshots (the walker's teal body counted under its nameplate, and the nameplate moving the way it walks), the walker drawn inside its own corridor before it is drawn in the Commons, and the walker seeing the game there. The last needed one more log line in second-player.js, `player present`, the other players its welcome's snapshot holds: a player standing still sends no updates, so the relay's word in the welcome is how the walker sees them.
- **Neighbours are drawn, render only** (src/ship/neighbours.rs, called from `ShipStructure::generate_meshes`). Every plot but the one the home stands on is drawn as its kind's SHIPPED design (`HomeDesign::built_in`, never this player's own edited home), with its door corridor and the hole that corridor makes in its own shell and in the shared zone it runs to. No machines, no rooms (so no lights, no "you are in" room, no sealed volume) and no collision: the hole in First Street's wall is cut in its mesh only, so the wall still stops anyone walking into a neighbour's corridor. The design said "no collision"; this is where that rule bites, and the reason the street's collision is not cut.
- **The hull wraps every plot**, whoever holds it (src/ship/hull.rs `hull_geom`), so the hull is the same in every player's game and no neighbour's home stands out through the plating. Not in the plan: until this increment the other plot was empty, so nothing showed it.
- **`tile_home_clones` is gone, with its walkway connectors and the `corridor_types` registry (data/blueprints/corridor_types.ron) only they read.** Its bake lives on as `HomeStructure::bake_shell_groups`, the neighbour shell, keeping the BUG-045 fix (floors, ceiling and trim are part of the shell). A residential zone draws nothing; its homes are the plots.
- **A guest draws no home of its own: the home is PUT AWAY** (`ShipStructure::put_home_away`). The plan's wording allowed drawing nothing or the default shell on the default plot. Drawing nothing is not enough by itself, because the home's machines, animals, plants, built pieces, vehicles, hologram and showroom stage are all things the world draws where they stand; hiding each one at its own draw site would be a dozen gates, and removing the home zone would send its machines to the ship's first zone (the Commons) and lose their state (`sync_machine_entities` despawns a machine it no longer places). So a guest's home is moved, with everything it holds, by the same carry a welcome uses to move a home between plots (1b's `move_home`, `follow_home_box`, the machine shifts), to `HOME_AWAY_ORIGIN`: a kilometre off the ship's plan and 200 m below its deck, a footprint nothing of the ship shares (the carry tests x and z only). There it is drawn as no zone, walked into by no one (wall_collision), lights nothing (`home_lights`) and is wrapped by no hull; every plot, the default one included, is then drawn as a neighbour's. Nothing is lost or reset: it comes back the same way when the game leaves the shared world (stepping out, a refusal, a server switch, a dropped connection; `bring_home_back`, onto the plot remembered for the server it now talks to, else the default plot), or onto a plot a later welcome gives. A guest cannot build aboard (`outside_own_plot`) or open the build editor, and each says why in one sentence. What still draws at the place it is kept (its machines, animals and plants) is out of every view from aboard.
- **The remembered plot** (AppConfig `home_plots`, `RememberedPlot { ship_hash, plot }`, keyed by the server's normalised URL). Each applied welcome teaches it (`plot_memory_after`): a Move or a Stay remembers the plot, a guest welcome, a plot given back as one the home does not fit, and an account erased forget it; a refusal over the ship leaves it. The world load builds the home on it before it joins (`assemble_for_boot`, `boot_plot`), so a returning player's welcome is a Stay. Thought through:
  - *A server that forgot or released the plot*: the boot builds on the remembered plot, which may now be someone else's for a moment, and the welcome corrects it (a Move to the new plot, or a guest's put-away). Nothing is sent before the welcome is applied (1b's `game_welcomed`), so nobody sees the player there.
  - *A changed ship hash*: a plot id names a place only on the same ship, so a memory from another ship is not trusted (the default plot), and the next welcome overwrites it.
  - *"Start every session from the default home"* (on by default): that setting is about what the home holds, not where it stands, so the remembered plot still applies; the fresh home is built on it.
  - *An old config.json* has no `home_plots`: `#[serde(default)]` reads it as empty, so it loads, and its first world entry builds on the default plot as in 1b.
- **The rig boots the game a second time** (`reboot_*` checks, not in the plan): after the 1b legs the game steps out, quits, and boots again against the same relay the same way it came in; its world load must build the home on the plot it held, and its welcome must only confirm it. Only walker-first tells a remembered plot from the default (the game holds p2 there); the judge says so in game-first.
- **The probe reports** `boot_plot` (the plot the world load built on), `last_welcome` ("stay", "move", "guest" or "refused") and `home_away`.
- **The rig, as run (2026-10-04, release builds of the ship-homes-2 branch):** `--plots` 56/56 in both orders (runs 20261004-085131-plots-walker-first and -game-first), `--plots --entry menu` 56/56 in both orders (20261004-084312-plots-*), and the default rig 21/21 (20261004-085912). Each run also leaves `neighbour.png`, a picture (not judged) of the neighbour's plot from the zone its corridor runs to: the hole in the zone's wall, the corridor, and the default home's door with its interior dark (a neighbour has no rooms, so no lights). Three things the first runs taught the rig, each a rig fault, not the game's:
  - *The first-run privacy window* covered the meeting line in the autopilot's walker-first game, and both pictures read "0 teal px under its nameplate" (runs/20261004-081513-plots-walker-first, every other meet_* check passing): the default rig's step that answers it (`clearFirstRunWindows`) now runs at the meeting too, judged as `meet_view_clear`.
  - *A screenshot holds the game for one long frame* (465 ms with another session compiling on the same machine), the figure's buffer runs dry, and the frames after it ease back at up to 1.86 m/s for a 1.4 m/s walk (runs/20261004-082314-plots-walker-first): the meeting's walk is now judged on the first forward leg after the pictures (`meet_leg_recorded`), as the walk at home is judged on a forward leg.
  - *The capture can land a second after the nameplate is asked for*, and the figure walks 200 px in that time (asked at x 1505, captured at x 1286, runs/20261004-083557-plots-game-first): the rig asks again right after the capture and `figurePixels` counts the box spanning both.
  - One run (20261004-083557-plots-walker-first) also met a 1.3 s freeze of the whole game process while another session compiled, logged by nothing in run.log: the walk after it caught up at 4.9 m/s and failed `meet_steady_speed`, `meet_no_jump` and `meet_on_time`. The rig ran it again on a quieter machine and it passed; a freeze like that is what the smoothness checks exist to catch, so they were left as they are.
- **Every new rig check was seen to fail.** On copies of a green run re-judged with `--dry-verdict` and one thing broken each (the relay refusing the last move, one 130 m jump, the walker never in its corridor, the walker never seeing the game, pictures with no figure, a second boot built on the default plot and moved), each failed its own check and no other. On a build: with `boot_plot` ignoring what was remembered (the 1b world load, always the default plot), walker-first failed `reboot_built_on_its_plot` ("the second boot built the home on p1 before joining; it held p2") and `reboot_welcome_confirms` ("its welcome did \"move\"") and nothing else, 54/56 (runs/20261004-090853-plots-walker-first); the green exe was put back and the tree checked fresh after.
- **Left for later:**
  - a neighbour's home is always the default design; drawing each player's own published design is a later step (homes are not shared yet);
  - a guest's home at the place it is kept is still simulated (its garden grows, its machines run) and its machines are drawn there, out of view from aboard; seen from outside the ship (Dev travel), they float under it;
  - the default rig (`just verify-copresence` without `--plots`) still stands the camera in p1's vehicle bay and has the walker walk straight there from p2, through the neighbour's walls; the walk it judges is unchanged.

### Increment 3: the relay's world becomes this ship

What changes:
- The relay's rooms come from the ship's shared zones instead of data/ships/starter_fleet.ron.
- The crew's chores move to the Commons and mess hall.
- The explore quest visits shared places and gains a "find your home" step that reads the plot.

Files:
- `src/relay/handlers/game_state.rs` (313-418, 581-631, 1217);
- `data/npc/chores.ron`;
- `data/npc/crew.ron`;
- `src/relay/features.rs`;
- the rig, which records the crew figures.

Proof: no chore target lies inside a plot box, and no crew figure is ever drawn inside a plot.

### Increment 4: getting around at ship scale

What changes:
- the speed check and the correction message (section 5.10);
- transit links with stable ids. This also fixes the teleporter at structures index 3, which is paired with index 5, a ladder (ship_structure.ron:1421-1461);
- "aboard" becomes "inside the ship's bounds" (lib.rs:3424);
- game delivery by zone (relay.rs:28-62);
- an air volume per home (home_spawn.rs:62, survival_env.rs:270-281).

Files:
- `src/relay/handlers/msg_handlers.rs` (3285-3296);
- `src/relay/relay.rs`;
- `src/lib.rs`;
- `src/engine/home_spawn.rs`;
- `src/engine/survival_env.rs`;
- `src/ship/ship_structure.rs`.

Proof: a relay test for the speed check, and a rig run that sends one oversized jump and sees a correction instead of a freeze.

### Increment 5: building only on your own plot

What changes:
- the stopped shared-building increment, in plot frames (section 8 below);
- `may_build` and `may_remove` as in section 4;
- the permit certificate, verified in `src/relay/core/pq_crypto.rs`;
- ShipStructureEditing switched off while joined unless the relay grants `can_edit_ship`. That is a column added to the roles table through the ALTER block, with no index before it (BUG-046 rule).

Files: shared-building.md section 8's list, plus:
- `src/config.rs` (158-170);
- `src/relay/storage/mod.rs` (roles, 1229-1245);
- `src/relay/core/pq_crypto.rs`;
- `src/net/dm_pq.rs` (permit building, beside `build_friend_cert`).

Proof: verify-shared-build (shared-building.md section 7), plus a refused build inside the other player's plot.

### Increment 6: the starts as data, the Cabin, the mess hall

What changes:
- **NEW data:**
  - `data/homes/starts.ron`, with three rows: design, plot kind, default engagement modes per domain, shared facilities;
  - `data/homes/cabin.ron`, with a single-storey cabin block plot.
- **Zone types:** `mess_hall`, `recreation`, `cabin_block`, `apartment_block` and `street` in data/blueprints/zone_types.ron.
- **The picker.** "Choose where you live" replaces Settings' Home Design (settings.rs:3585-3602), built in native first, then mirrored on the web. `game_join` carries the requested start.
- **The mess hall** serves free, metered crew meals.
- **Saves.** The home is stored per save, not per install. WorldSave gains a plot id and a ship id (src/persistence.rs:12-49).

Files:
- the data files above;
- `src/gui/pages/settings.rs`;
- `app/web/pages/settings-app.js`;
- `src/persistence.rs`;
- `src/relay/handlers/msg_handlers.rs`;
- `src/relay/storage/plots.rs`;
- the food system that serves meals.

Proof: the walker joins as a Cabin, and the rig sees it reach the mess hall.

### Increment 7: stacked homes and decks

This is the gate for any multi-storey Cabin block or apartment above the ground floor.

What changes:
- collision per storey (src/ship/wall_collision.rs:1-11, 130-134);
- footing chosen by height (src/lib.rs:3697-3706);
- lifts that climb more than one storey (data/blueprints/structure_types.ron:28-46);
- teleport jumps that set y.

### Increment 8: holdings, lockers, trade escrow

What changes:
- the `holdings` table;
- `game_deposit` and `game_withdraw`;
- confirm moves the offer into `trade:<id>`, cancel moves it back, and complete moves it to the other player. This turns BUG-114 and BUG-115 into facts the server holds;
- the mess hall's `stock:<zone>`;
- purge completed trades;
- correct the trading.rs:1-3 comment.

Files:
- NEW `src/relay/storage/holdings.rs`;
- `src/relay/storage/trading.rs`;
- `src/relay/handlers/msg_handlers.rs` (2796-2846);
- `src/gui/pages/trade.rs`.

### Increment 9: shipments, version 1

What changes:
- the `shipments` table;
- the lines data file;
- the once-a-second sweep;
- courier robots;
- parcel lockers at the mess hall;
- the arrival-time view.

Simplified mode comes first. Player-carried goods need increment 4.

Files:
- NEW `src/relay/storage/shipments.rs`;
- `src/relay/mod.rs` (645-677);
- `src/relay/handlers/game_state.rs` (the chore cycle);
- NEW `data/transit/lines.ron`;
- `data/transportation.ron` (corrected speeds);
- the client's locker and arrival-time UI.

### Increments 10 to 14

| Increment | What it is | Notes |
|---|---|---|
| 10 | The Apartment block, its common house, the rota | Cooperative food. Shared utilities need the substation tier (S3), which is not built |
| 11 | Transit | Rideable rail cars on the rail graph (empty today, ship_structure.ron:1744), timetables, spoke lifts, the tube network, full-realism capacity |
| 12 | Hangar departures and seats | Needs increment 4 |
| 13 | Planner readouts | Section 6 |
| 14 | Between motherships | Section 5.9 |

---

## 8. What this changes in docs/design/shared-building.md

That file is untracked and already carries a "SUPERSEDED IN PART (2026-10-03)" header (shared-building.md:3-9).

**What changes:**
1. **The frame.** `frame = "home"` (every home at the same coordinates) is retired.
   - Pieces use `plot:<id>`, with poses relative to the plot, so today's home coordinates stay valid for p1.
   - `zone:<id>` is reserved for shared spaces and needs `can_edit_ship`.
   - `site:<id>` for planets is unchanged.
   - The "Where players meet" options in its section 6 collapse to this one answer.
2. **Co-op trust becomes enforced location.**
   - `may_build` = the builder owns the plot or holds a permit, and the piece's box lies inside it.
   - `may_remove` = the owner, or the permit holder who placed it, or `is_game_admin`.
   - Refunds go to the owner.
   - This was its section 6 "enforced rules" path. Only the location half is decided; materials stay on the client's word until increment 8.
3. **Its status line.**
   - "Where players meet": answered, anywhere aboard, in shared places.
   - "Co-op trust vs enforced rules": answered for location only.
4. **Snapshots per plot.** With homes spread across the ship, a join should send the pieces of plots near the player, not all 2,048 at once. That is the interest management already listed as "later" in its section 9.
5. **Its place in the order:** after increments 1 to 4. It is increment 5.
6. **What survives unchanged:**
   - the Wave 0 contract;
   - the seq and snapshot protocol;
   - `SharedPiece`;
   - save separation;
   - the seven `shared: true` blueprints;
   - the bandwidth budget;
   - the file waves in its section 8, to which increment 5 adds the plot lookup and the permit check.
7. **Its test plan** gains one case: a build inside another player's plot is refused with `not_allowed`.

---

## 9. Open questions for the operator

Each question has a recommendation.

1. **Names, and which kinds the first picker offers.**
   - DECIDED 2026-10-03: the full list of home kinds (section 1.2b), the Homestead as the default start.
   - Still open: whether the first "Choose where you live" offers every kind or some are reached by moving house. Recommend offering all, Homestead highlighted.
   - Recommend "Choose where you live" and "Moving house" as the screen words.
   - Recommend **"plot"** as the one word for a home's place, and renaming the "berth id" in room-purpose-architecture.md:102 to match.
2. **The Cabin's washroom.** Recommend shared per block of 40 or fewer, as real berthing does. The alternative is a private wet room, about 16 m2 in all.
3. **Earned or free?** Recommend free: move any time a home is free, with a cooldown.
4. **When a kind of home is full.** Recommend a waiting list, plus blocks an admin opens from premade designs, plus a new deck, drum section or mothership when the ship is truly full. Never a lottery, never invisible copies.
5. **Absent owners.** Recommend never demolishing. Mothball into storage only when the ship is full, someone is waiting, and the owner has been away longer than a Server Settings period.
6. **A server home versus your offline home.** Recommend that a server gives you a fresh home of your chosen start, and your offline home stays offline. An open server may let you bring your design: the layout, not its contents.
7. **Trust inside a home.** Recommend owner plus permit holders, as in section 4. The relay checks and charges materials once holdings exist, and Blocked item 5 stays open on materials until then.
8. **Block shared areas.** Recommend not editable at first, garden planting open to residents, and a resident vote later.
9. **Mode and Rank.** Recommend ratifying `can_edit_ship` as a rank the relay grants. Local Dev mode in a shared world would no longer edit the ship.
10. **Teleporters.** Recommend never for goods. For people: Dev or rank only now, and perhaps later between transit hubs at an energy cost.
11. **The clock, and free meals.**
    - Recommend that people and goods move in real seconds whatever the world clock speed, while growing runs on the game clock. This joins the open 1x versus 72x question: at 72x a 20-minute rail trip spans a game day.
    - Recommend a free, metered basic crew meal for everyone, matching the reactor decision.
12. **Do NPC residents eat from the same stores?** This is his open taste call. Recommend yes, as aggregate flows per zone, which is what makes the mess halls' supply lines matter.
13. **Two civic tiers.** Recommend yes: a neighbourhood node with a common house or mess hall, and the district Concourse.
14. **The planning numbers.** Recommend 5 minutes' walk from every home to a mess hall, 15 minutes to every daily need, and a minimum distance from homes to industry. That distance is his number to set; this document has no source for one.
15. **Hangar or home bay.** Recommend the home bay keeps ground vehicles and drones, and spacecraft live in the shared hangar.
16. **Goods between relays.** Recommend off by default, switched on per pinned peer by the server admin.
17. **Retire the Pioneer** from the relay and move the crew into the shared places (increment 3). Recommend yes.
18. **Children.** Recommend offering "Cabin + simplified mode" as a suggested pair at the picker, while keeping them two separate settings.
19. **When does an idle plot go back?** Today a plot is kept until an admin releases it (Server Settings > ADMIN > Homes on the ship) or the player's home does not fit it; nothing releases one by time, so on a busy relay the ship fills with the plots of people who left. This is a policy only the operator can set, and it meets question 5 (absent owners): recommend releasing only after a Server Settings period of absence, only when someone is waiting for a plot, and telling the returning player plainly that their plot went back (their home itself lives in their own save, not on the relay, so nothing of it is lost).

---

## Appendix: stale or conflicting items found while writing this

- **Sources:**
  - docs/design/self-sufficiency.md:82 gives 700 to 1,000 m2 of food land per person with no source; Ecology Action publishes 260 to 316 m2. Settle it in a dated findings document.
  - data/transportation.ron's speeds carry no sources, and the AGV and tube figures exceed real ones (section 5).
  - mothership-superstructure.md:68-86 uses Dunbar layers without a source, and the figure is disputed.
- **Out-of-date docs:**
  - gravity-and-movement.md:322-344 uses the retired 72x clock in its travel table.
  - ROADMAP.md:336-338 and mothership-superstructure.md still list "one editor or two" as waiting on the operator. He answered it on 2026-09-15 and 2026-09-19.
- **Code comments:** trading.rs:1-3 says items are "locked in DB"; the relay holds none.
- **Data:**
  - Clone slot 0 of `res-1` overlaps the Commons and its corridor today (computed from ship_structure.ron:1653-1658, 1749-1757, 2656-2665).
  - The home's teleporter at structures index 3 is paired with index 5, which is a ladder (ship_structure.ron:1421-1461). Links will need stable ids.
- **Uncommitted edits:**
  - The edit to PRIORITIES' "Waiting on the operator" list drops "co-op trust or enforced rules". His words settle only where a player builds.
  - PRIORITIES Day 4 (docs/PRIORITIES.md:88-108) and journal entry 108 were uncommitted working-tree edits when read.
- **Not researched:**
  - a sourced handling time per freight transfer;
  - washroom and corridor ratios for cabin blocks;
  - mess hall seats per resident;
  - a walking speed for children;
  - travel times between motherships, which wait on fleet geometry.