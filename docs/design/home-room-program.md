# The home room program: which rooms a home needs, and why

Status: **proposal for the operator's review, 2026-11-13.** Phase 1 of the new homestead: the room
program and two drafts on the decided plots. Nothing here is home-design data yet; the data files,
their tests and the dev-start kinds come after he approves (section 7).

The operator, 2026-11-13, redirecting the Fibonacci-square homestead: "What if we instead go the
route of figuring out which rooms we actually need. Like they're unique enough to justify their own
room. Like the kitchen or bedroom." So every room below earns its walls with a function its
neighbours cannot share, and the layouts follow the program rather than a ladder of squares. The
plots stay as decided ([mothership master plan](mothership-master-plan.md), section 1.5): **34 x 55
m for a household, 21 x 34 m for one person.**

Sizes come from [the sizes research](../reference/research/2026-11-13-mothership-sizes/README.md)
and the sources it cites (detail in its
[housing-tiers notes](../reference/research/2026-11-13-mothership-sizes/C-housing-tiers.md)), from
two dated findings ([noise and sleep](../reference/findings/2026-11-06-noise-and-sleep.md),
[stairs](../reference/findings/2026-10-20-stairs-guards-and-headroom.md)) and from the
[survival garden](survival-garden.md). Where no source gives a size, the number is marked
**judgement** and listed in section 6 for the next research pass.

## 1. The test for a room

A function gets walls of its own when at least one of these holds:

- **Services**: it needs water, drains, extraction or power its neighbours do not (cooking, bathing,
  laundry, the ship's life-support hookups).
- **Separation**: it must be kept apart for quiet, dark or privacy (sleeping), for noise, dust and
  fumes (a workshop), for hygiene (animals, food storage), or for a climate a house must not have
  (a greenhouse's heat and humidity, a mushroom room's damp dark).
- **A threshold**: it is where dirt, weather or noise stops (an entry, a mudroom).

Everything else shares a room with something it suits, and section 3 says which and why.

## 2. The rooms that earn their walls

| Room | Why it gets walls | Minimum | Comfortable (as drawn) | Basis |
|---|---|---|---|---|
| Bedroom, double | Sleep needs privacy, quiet and dark: 30 dB LAeq inside a bedroom at night and at most 1 lux in the sleeping room (noise finding) | 11.5 m2 | 16 m2 with a walk-in wardrobe beside it (homestead), 12.25 m2 with a desk (one person) | UK space standard: "a double (or twin bedroom) has a floor area of at least 11.5m2" |
| Bedroom, single | The same | 7.5 m2, 2.15 m wide | 10 m2 (4 x 2.5 m) | "a single bedroom has a floor area of at least 7.5m2 and is at least 2.15m wide" |
| Bathroom or shower room | Water, drains, steam extraction, privacy | about 4 m2 (judgement: shower, WC, basin) | 8 to 10 m2 | judgement |
| Kitchen (with dining) | Plumbing, extraction, heat, food hygiene; its smells and clatter | for one person the living room, kitchen and dining together about 24 m2 after a small hall (computed below) | 27.5 m2 kitchen and dining for five; 30 m2 living, kitchen and dining for one | UK space standard arithmetic, judgement |
| Living room (households only) | Screens, guests and a quiet evening while someone cooks: two activities at once | about 18 m2 for five (computed below) | 22.5 m2 | arithmetic, judgement |
| Entry | Where the street's dirt and noise stop; coats and shoes | about 3 m2 (judgement) | 4 m2 | judgement |
| Utility: mudroom, laundry, life support | The ship's air, water and power hookups need a room a technician can reach; laundry needs water and drains; boots from the garden and workshop need a bench and a sink; all of it is noisy or dirty and wants the back door | about 6 m2 (judgement) | 12 to 15 m2 | judgement |
| Harvest store | Food keeps in a cool, dark, pest-tight place; a homestead stores a season's roots, grain and preserves, more than cupboards hold | 4 m2 for one | 12 m2 for five | judgement |
| Mushroom room | Mushrooms want dark, damp, still air, apart from the greenhouse's lamps; it carries the vitamin D shelf (decided, survival garden) | 4 m2 for one | 12 m2 for five | judgement |
| Greenhouse | Crops need light, warmth, humidity and water a house must not have | **122.2 m2 a person** (the comfortable garden with paths, decided 2026-11-07) | 616 m2 for five, 144 m2 for one | survival-garden.md; soil instead would take 372 m2 a person (Ecology Action) |
| Workshop | Noise, dust, fumes and tools: the home keeps to 35 dB by day (noise finding) | about 20 m2 of benches (judgement) | 5 x 10 m (homestead), 4 x 8 m (one person) | judgement |
| Vehicle bay | A big door, charging and room to work round a vehicle; shares the workshop's dirt and tools, so it is the same room | 4 x 8 m for one vehicle (judgement) | 7 x 10 m for two | judgement |
| Hen house | Hygiene (droppings, ammonia, salmonella) keeps birds out of the house; they need perches, nest boxes and shelter | 0.14 m2 a hen inside: "a minimum of 1.5 sq. ft. (0.14 sq. m) per hen" (Certified Humane) | 0.9 m2 a hen (3 x 3 m for ten) | Certified Humane; 0.28 to 0.37 m2 extension advice |
| Hen range | Room to scratch and forage, under the orchard's trees | **10 m2 a hen** (Certified Humane Pasture Raised: "2.5 acres (1 hectare)/1000 birds") | 123 m2 for ten, 62 m2 for six | Certified Humane |
| Rabbit hutch and run | Hygiene and shelter, apart from the hens | 3 m2 a breeding doe (0.6 inside, 2.5 outside; EU 2020/464) | 12 m2 for a buck and three does, 6 m2 for a pair | EU 2020/464 |
| Hives | Bees pollinate the garden and the orchard; their flight path is kept off the paths | a hive and its clearance | 2 hives in 12 m2 (homestead), 1 in 6 m2 | Oregon State: 4 colonies a hectare of fruit trees |
| Stair (two-storey homes) | The way up | 1.0 m wide, 3.81 m of flight for a 3.0 m floor: 16 risers of 187.5 mm, 15 goings of 254 mm, landings at least 0.914 m deep, 2.032 m headroom | as minimum | stairs finding (riser at most 196 mm, going at least 254 mm) |

**The arithmetic behind the kitchen and living rows.** The UK space standard sets whole-home floor
areas, not room areas. A one-bedroom home for one person is 39 m2 (37 with a shower room); less a
7.5 m2 bedroom, 1.0 m2 of built-in storage and about 4 m2 of shower room leaves about 26 m2 for the
living room, kitchen, dining and hall together. A three-bedroom home for five on two storeys is 93
m2; less 30.5 m2 of bedrooms (two doubles and a single), 2.5 m2 of storage and about 6.5 m2 of
bathroom and WC leaves about 53 m2 for living, kitchen, dining, halls and the stair. The rows
above split those remainders; the drafts are larger, as the comfortable column says.

## 3. What shares a room, and why

- **Dining with the kitchen**: the table is served and cleared from the kitchen, and one set of
  services does both.
- **Living with the kitchen, for one person**: one person is never cooking while someone else wants
  quiet, so the separate living room's reason is gone. Households keep it separate, with a wide
  sliding door so the two rooms open into one for gatherings.
- **Study with the bedroom**: a desk needs the quiet a bedroom already has. (The co-housing common
  house adds a quiet room for those who want one.)
- **Dressing room with the bedroom**: a wardrobe, walk-in for the main bedroom.
- **Mudroom, laundry and the life-support plant in one utility room**: all three need water and
  drains, all are noisy or dirty, and all belong at the back door.
- **Storage as cupboards, not rooms**: the standard's built-in storage (1.0 m2 for one person, 2.5
  to 3.0 m2 for five or six) is the main bedroom's wardrobe and an upstairs linen cupboard.
- **Pantry**: a larder cupboard in the kitchen, and the harvest in the harvest store by the
  greenhouse, where the food comes in.
- **The screens den (today's console room) with the living room.**
- **Guest room** with the spare bedroom (homestead), or the common house's guest rooms (co-housing).
- **Seedling room, potting bench and the aquaponic tank in the greenhouse.**
- **Forge** in the workshop under a hood, or the neighbourhood's workshop hall.
- **Barn**: the hen house and rabbit hutch. Goats are not homestead animals: a goat needs 809 to
  1,214 m2 of pasture and is a herd animal, so the herd lives in the farm band (master plan 1.5,
  item 7).
- **Fields** become the orchard's beds; **power** comes from the ship's reactors, its panel in the
  utility; **exercise** is the living room or the sector's gym.
- **No lift**: the main bedroom and a shower room are on the ground floor, so the whole home works
  step-free for someone who cannot climb.

## 4. The program per tier

P = private to the home, S = shared, with whom in brackets. Areas per person are the research's
gross figures.

| Function | Barracks | Dormitory | Co-housing cluster | Row house | Homestead (up to 5) | One-person homestead |
|---|---|---|---|---|---|---|
| Sleeping | S: a bed space and locker in a bay of about 50 (6.7 m2, 9 recommended) | P: a single room (7.5 m2) | P: bedrooms | P: bedrooms | P: a double and three singles | P: a double |
| Bathing | S: washrooms | S: per floor | P | P | P: a shower room and a bathroom | P: a shower room |
| Cooking and eating | S: the sector's mess hall | S: a kitchen-lounge per floor | P, plus S: the common house's kitchen and dining for shared meals | P | P | P, with living |
| Living | S: a day room | S: lounges | P, plus S: the common house | P | P | P, with the kitchen |
| Entry and utility | S | S: laundry | P entry; S: laundry in the common house | P | P: entry and a utility room | P: one entry and utility |
| Workshop and vehicle | S: the neighbourhood's hall | S: the hall | S: the cluster's workshop | S: the hall | P | P |
| Growing | S: the shared garden, tended by all | S: the ship's farms | S: the cluster's garden, tended by whoever wants to | P: a yard of at least 25 m2; the ship's farms | P: greenhouse, orchard, beds | P: the same, for one |
| Animals | S: the farm band's herd | S | S | S | P: hens, rabbits, bees | P: the same, fewer |
| Guests and play | S | S | S: guest rooms and a playroom in the common house | P | P | P |
| Floor a person (research) | 13 to 20 m2 | 31 m2 | 33.7 m2, site 88 m2 | 19.8 m2, plot 25 m2 | house 33 m2 at five, plot 374 m2 at five | house 66 m2, plot 714 m2 |

### The two plots, totalled

| Use | Homestead, 34 x 55 m | One-person, 21 x 34 m |
|---|---|---|
| House, ground floor | 110 m2 | 66 m2 |
| House, upper floor | 60 m2 (56 of rooms and halls, 4 of stairwell) | none |
| Workshop and vehicle bay | 120 m2 | 64 m2 |
| Greenhouse with its harvest store and mushroom room | 640 m2 (616 growing, needed 611) | 152 m2 (144 growing, needed 122.2) |
| Orchard, hen range, hen house, rabbits, compost, hives | 646 m2 | 336 m2 |
| Yard | 110 m2 | none (the terrace) |
| Paths, terraces, front garden | 244 m2 | 96 m2 |
| **Plot** | **1,870 m2** | **714 m2** |

The homestead's house is 166 m2 of rooms and halls for five people in four bedrooms, against the
space standard's 93 m2 for three bedrooms and five people and 106 m2 for four bedrooms and six. The
one-person house is 66 m2 against the standard's 39 m2.

## 5. The two drafts

Both plots put the home at the street, the workshop beside it with its big door on the street, the
greenhouse behind, and the orchard and animals at the back, so the hens are at least 15 m from the
house (31 m on the homestead) and the greenhouse is visited without passing through the hens. A side path reaches the
orchard without crossing the greenhouse. Two fan lines carry air: one over the orchard (trees need
"a steady breeze plus gusts", master plan 1.5, item 12) and one along the greenhouse.

**Homestead, ground floor (house 11 x 10 m):** living room 5 x 4.5 m (22.5 m2), kitchen and dining
5 x 5.5 m (27.5 m2), entry 2 x 2 m (4 m2), hall and stair 2 x 8 m (16 m2), utility 4 x 3 m (12
m2), shower room 4 x 2 m (8 m2), main bedroom 4 x 4 m (16 m2), wardrobe 4 x 1 m (4 m2). Beside it a
yard of 11 x 10 m and the workshop and vehicle bay of 12 x 10 m; behind, a 3 m terrace, the
greenhouse (616 m2 of growing floor, a 4 x 3 m harvest store and a 4 x 3 m mushroom room), then the
orchard (478 m2), the hen range for ten (123 m2) with a 3 x 3 m hen house, rabbits (12 m2), compost
(12 m2) and two hives (12 m2).

**Homestead, upper floor (over the 6 x 10 m east wing, floor at 3.0 m):** bathroom 4 x 2.5 m (10
m2), bedrooms 2, 3 and 4 each 4 x 2.5 m (10 m2), upper hall 1 x 7 m, landing 2 x 3 m, linen
cupboard 1 x 3 m, and the 1 x 4 m stairwell behind a 1.0 m guard.

**Why the upstairs is three single bedrooms over the quiet wing.** The planning sheet counts 2.5
people a homestead (`people_per_home`, data/ship/space_program.toml), an average, so many homes
hold four or five: the house is drawn for five, a double and three singles, one room for each child
of a couple with three. The main bedroom and a shower room stay downstairs so the home works
step-free. The other bedrooms go up rather than spreading the house, because 56 m2 more footprint is
56 m2 less garden. They sit over the main bedroom and shower room rather than over the living room
and kitchen (the first brief's idea), for three reasons: quiet sits over quiet at night (the
bedroom limit is 30 dB), the upstairs bathroom sits over the utility and shower room so the water
and drains run in one stack, and the living room and kitchen keep a single storey that can take a
high ceiling. It still exercises a real upper floor, as the operator asked on 2026-11-06 ("we
still want to allow multiple stories so that we can get multiple floors working").

**One-person homestead (house 11 x 6 m, one storey):** shower room 3.5 x 2.5 m (8.75 m2), bedroom
and desk 3.5 x 3.5 m (12.25 m2), living, kitchen and dining 5 x 6 m (30 m2), entry and utility 2.5
x 6 m (15 m2), a through-room from the front door to the garden door. Beside it the workshop and
vehicle bay of 8 x 8 m; behind, the greenhouse (144 m2 growing, 2 x 2 m harvest store and mushroom
room), then the orchard (252 m2), six hens on 62 m2 with a 2 x 2 m hen house, a pair of rabbits, a
hive and compost.

**Where Fibonacci is used:** only in the two plots, 34 x 55 m and 21 x 34 m, which were decided and
fit their needs (the research sized them from the food each household grows). No room, band or
building inside follows the ladder: every size above comes from the program.

The drafts were drawn by a small script from a layout file, so their numbers are the layout's own.
The same script checks that each plot is tiled with no gap or overlap, every door sits on a wall
between two rooms, every room is reachable from the street (the upper floor by the stair), and
every bedroom meets the space standard; each check was made to fail on purpose first.

## 6. Sizes no source here gives yet

Kitchen, bathroom, entry, utility, workshop, vehicle bay, harvest store, mushroom room and the hen
house's footprint beyond the per-hen minimum are judgement. Before any design is called "to code"
(master plan 1.3), a dated finding should collect room minimums and fire rules per country (the UK
space standard and Building Regulations, the US International Residential Code, Japan's Building
Standards Act). One rule to check there: whether a bedroom may open off a kitchen-living room (the
one-person house does; its bedroom has a window to the garden).

## 7. Open questions, and what follows approval

1. **The household the homestead is drawn for**: five people, four bedrooms (recommended), against
   the sheet's average of 2.5.
2. **The upstairs**: over the quiet wing (recommended) or over the living room and kitchen.
3. **One person's living room and kitchen as one room** (recommended).
4. **Bees**: a hive or two on every homestead (recommended for the self-sufficient tier) or one
   apiary per neighbourhood.
5. **The vehicle bay**: two vehicles for a household, one for one person (recommended); anything
   larger belongs in the hangar.
6. **The garden**: the decided lit greenhouse for the staple diet, with soil beds in the orchard for
   variety and fodder (recommended, as drawn).
7. **The denser tiers' drafts next**: the row house first (recommended; the operator liked row
   houses), then the co-housing cluster.

After approval: the two designs become home-design files in `data/homes/` (the existing shape:
walls with their openings, floor slabs for the upper floor, zones carrying each room's
`room_type`, lights, the stair), selectable as new kinds in `data/homes/starts.ron` without
replacing the default; today's homestead stays loadable until the wipe decides. Its console room
zone (`console-room`) is referenced across code, scripts and fixtures, so merging it into the
living room is part of that build. The drafts should then be drawn from those files, so the
drawings and the game can never disagree.

## His answers (operator, 2026-11-13)

1. **Five people, four bedrooms.** "Let's do 5 people with four bedrooms. That's akin the household I grew up with 2 parents and 3 kids (one being me)."
2. **The upstairs over the quiet wing.** "Plumbing in one stack makes sense. Let's do it over the quiet wing."
3. **One room for one person's living, kitchen and dining.** "Make them one room."
4. **Bees on every homestead.** "Each homestead gets its own hive."
5. **The vehicle bay: two vehicles for a household, one for one person, bigger craft in the hangar.** "Works for me."
6. **The lit greenhouse for the staples, soil beds in the orchard for variety and fodder.** "Agreed."
7. **The row house next.** "Let's start with row house."

So the two drafts are approved as drawn; phase 2 turns them into home-design data (section 7, "After approval") and drafts the row house.
