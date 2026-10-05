# Since the last public update

A running list of what has shipped since the last posts went out, kept so the
next update can be written from it without digging through the logs. The last
posts were the evening update of 28 September 2026
(`docs/outreach/posts/2026-09-28-evening-update.md`, v0.1396.1 to v0.1421.0).
When the next update is written, move this list into it and start a fresh one.

Each line is written the way a player would hear it; the release notes and
`docs/history/` hold the detail.

## Shipped

- **v0.1463.0: fires that burn, heaters that heat, and illness that makes sense.**
  Casting Campfire builds a real stone-ring fire that burns its logs and warms you
  when you sit near it (not through a wall, and never under a roof). The space
  heater now warms the air of the room it stands in. Food poisoning drains your
  body's water over a day or two instead of killing you in eight minutes, and
  medicine finally does something when you use it. Hunger and thirst now keep
  real time at any frame rate (before, a full stomach could stay full forever).
  Trees and you now stand on the ground at Silverdale instead of floating above
  it. Also: the generator burns its own fuel, no more money loops at the trading
  post, your server choice sticks, and three new Library guides on toilets, germs
  and infection.

- **v0.1462.1: three Library guides on germs and food.** Microbes, Good and Bad (which
  germs help and which harm, and what stops them); When Food or Water Makes You Sick (what
  to do, what to drink, the danger signs that mean a doctor, with the special rules for
  babies, pregnancy and shellfish); and The Chemistry of Keeping Food (why salt, acid,
  drying, cold and heat keep food safe, and where each stops working). Each was checked
  against its sources by a second, independent reviewer before it shipped.

- **v0.1462.0: room for twelve households aboard.** The mothership now has
  twelve homestead plots along a First Street over a kilometre long, so a shared
  server can give twelve players a home of their own; the thirteenth arrives as a
  guest and is told so. Players near each other see each other, and the far end of
  the street is out of sight from the Commons. Also fixed: two of the game's data
  files (docking ports and space infrastructure) were being silently ignored.

- **v0.1461.0: a real game from the first minute.** A fresh install now
  starts in Normal mode with your progress kept between sessions: things are
  used up, tools wear, and nothing is free. Your first ten minutes walk you
  through it: check your vitals, eat, plant, make a tool, bring in iron and
  smelt it, build a chest, and step out of your own front door, with a note
  after each step saying what is next. A new Death setting: Simplified loses
  nothing, and Realistic leaves everything in your backpack where you fell,
  marked on your screen, for an hour of play. Building in your own home now
  saves into your own save, and a machine you place costs its item (and comes
  back if you remove it).

- **v0.1460.1: nine Library guides on keeping food, getting through an
  outage, and getting along.** Water-Bath Canning, Pressure Canning, and
  Salting, Curing and Smoking; Choosing and Running a Generator, Emergency
  Shelter, and Pressure in Water, Air and Steam; Handling Conflict, The Law
  Where You Live, and Mental Health Under Strain (how to help someone who may
  be thinking about suicide, and where to call). Each was checked against its
  sources at least twice before it shipped. Drying Food now says freezing is
  not enough for wild game and cooks it to 165 F, and the game's Dry Meat
  recipe now says to heat the meat first. The Library now has 110 sourced
  guides.

- **v0.1460.0: the first hour gets stakes, and the shared world runs in real
  time.** A new player now starts alone in their own home instead of being
  dropped into the live shared world, every way out of the game saves first,
  and a one-time hint names the keys that matter. Your body is saved with
  your game, so quitting no longer heals you or fills you up; stored food ages
  wherever it is kept (the freezer slowest); the bedroom's own bed sleeps
  you. The one-person home can now finish the first quests (it has a smelter,
  a workbench and a trading post), quest steps that could never finish now
  can, the first step says where to get iron, and a finished quest tells you
  what you got and what is next. The drone stops after an empty trip and has
  a Stop button, gathering takes what fits in your pack and leaves the rest,
  a build you cannot afford says what is missing right where you aimed, the
  starting Barn holds coal, and the free showcase garden only grows while
  resources are free. On a shared server, a day is now a real day: the
  operator's decision, so nobody joining faces accelerated hunger.

- **v0.1459.1: six Library guides on heat, fire and first aid.** Heating a
  Home Safely (space heaters, a carbon monoxide alarm on every sleeping
  floor, the signs of carbon monoxide poisoning, and Washington's rule on
  portable kerosene and propane heaters indoors), Making and Controlling
  Fire (putting a fire out properly, "once out, stay out", burn bans),
  Fuels and Their Hazards (storing gasoline and propane, what to do if you
  smell gas), First Aid Until Help Arrives (CPR for adults, children and
  babies, choking, bleeding, seizures, low blood sugar), When Heat Becomes
  Dangerous (heat exhaustion against heat stroke, and how to cool someone)
  and Helping Your Neighbours After a Disaster (damaged buildings, gas
  leaks, CERT). Each was checked twice against its sources before it
  shipped. The Library now has 101 sourced guides.

- **v0.1459.0: getting around the ship, and a HUD that tells the truth.** On
  a shared server, the server now checks how far each player moves. A move
  that is too big puts the player back where the server last had them,
  instead of freezing them, and the honest fast moves are allowed: coming
  back after a respawn or a reconnect, closing the build editor on your own
  plot, teleporters, transit links, vehicles and ladders. Other players
  aboard appear when they come within 250 m and drop out past 300 m, so the
  server sends you only who you could see. Each home has its own air, up its
  own elevator and stairs. Nobody is told who lives on which plot. Also
  fixed: the Home Station marker now shows from the ground with its distance,
  the HUD says FLY whenever fly mode is on, the home's machines take their
  inputs from home storage and never from your backpack, and opening and
  shutting the build editor without an edit no longer rewrites the home's
  files.

- **v0.1458.1: six Library guides on electricity, power tools, finding
  your way and boats.** Electricity and How It Flows, Where Your Own
  Electrical Work Stops (proving a circuit dead, generators and carbon
  monoxide, what a homeowner may do in Washington), Power Tools (guards,
  kickback, grinders, chain saws), Navigating Without Instruments, What to Do
  When You Are Lost (stay put, calling and texting 911, signals, lightning),
  and Floating and Boats (life jackets, cold water, capacity plates). The
  Library now has 95 sourced guides. The fact checks also found three game
  bugs, now being fixed: the home marker never shows from the ground, the HUD
  says WALK after the Dev page's travel left fly mode on, and the home's
  sawmill eats logs from your backpack.

- **v0.1458.0: the fleet never runs out, and you can see what you use and
  give.** On a shared server the fleet's stores are unlimited for now, so
  nobody misses a meal. Every meal you take and the ship's power your home
  draws are counted as used, and anything you give the fleet at a ship store
  is counted as given, each at its value in credits. Inventory > The fleet
  shows whether you are in the black or in the red. Only you see your own
  ledger. A server admin can switch to a realistic mode where stores can run
  out.

- **v0.1457.3: three Library guides on water, power and keeping food
  cold.** Wells and Groundwater (never going down a well, what to do with a
  well after a flood, setbacks from a septic system), Water and Wind Power
  (how much power a stream or the wind can really give, why to design for the
  driest month, and keeping a grid-tied system from feeding the line in an
  outage), and Cold Storage Without a Fridge (the 40 F rule, root cellars, and
  why food never goes out in the snow). The Library now has 89 sourced guides.

- **v0.1457.2: three Library guides on your body and on pests.** How Your
  Body Works (vital signs, and the emergencies where minutes count: heart
  attack, cardiac arrest, stroke, sepsis, heatstroke, carbon monoxide),
  Invasive Species (what makes a species invasive, giant hogweed, cleaning
  boots and boats, and why firewood should not travel), and Rats, Flies,
  Mosquitoes and Ticks (cleaning up after rodents safely, traps and bait
  stations, and removing a tick). The Library now has 86 sourced guides.

- **v0.1457.1: six more Library guides, on animals and on making things.**
  How Animals Work, Keeping Animals (water, feed, fencing, hay fires, keeping
  new animals apart), and Animal Health and Disease (taking a temperature,
  bloat, which diseases you must report, why antibiotics now need a vet, and
  staying safe at birthing time); Pottery and Firing (lead in glazes, kiln
  safety, clay dust), Soap, Lye and Cleaning (handling lye safely, never
  mixing cleaners) and Making a Tool (handles, axes, grinders and why a file
  is too brittle for a pry bar). The Library now has 83 sourced guides.

- **v0.1457.0: build big things from what is in your home.** Crafting at a
  station now takes parts from your backpack first and then from your home's
  storage, and anything too big for your backpack goes into home storage when
  it is done. So you can build a boat, a car or a spacecraft pod from the
  materials you have stored at home, the way a real workshop works. The
  Crafting page shows how many of each part are in your backpack and how many
  at home, and says plainly what you are short of.

- **v0.1456.0: the shared world is the mothership, and its crew live
  aboard.** When you join a server, the world you share with everyone is now
  the mothership itself: the Commons, its mess hall and First Street. The
  ship's crew go about their work in the Commons and eat in the mess hall,
  from the same food stores players eat from, so a store that runs out means
  someone misses a meal. The first quest now ends with finding your own home
  on the ship.

- **v0.1455.1: three more Library guides on looking after a building.**
  Roofs and Keeping Water Out (shingles and flashing, finding a leak, snow
  loads, working safely at height and away from power lines), Repairing a
  Building (rot, damp and mould, leaks you cannot see, lead paint and
  asbestos before you start, and what to do if you smell gas), and Salvage
  and Reuse (reclaimed lumber, pallets, old drums and tanks that must never
  be cut or welded, treated wood, and old appliances). The Library now has
  77 sourced guides.

- **v0.1455.0: no more free money at the trading post, and vehicles built
  from real amounts of material.** 24 recipes let you buy the parts, craft
  something and sell it back for more, forever; none do now, and a test
  checks every recipe for it. Fixing that showed the vehicle recipes were
  toys (a 50-tonne freighter from 200 kg of parts), so every vehicle is now
  built from about its own weight in steel, aluminium, glass, wiring and
  parts, and priced to match: a motorcycle 2,500 credits, a sedan 14,500, a
  light mech 120,000.

- **v0.1454.1: three Library guides on building.** How a Building Stands Up
  (how loads travel to the ground, snow on a roof, warning signs that mean get
  out, and why connections fail first), Foundations and Ground (what soil can
  carry, frost depth, keeping water away, and digging safely beside a
  footing), and Framing a Building (studs, headers, bracing, nail gun safety,
  and checking for lead and asbestos before opening a wall). The Library now
  has 74 sourced guides.

- **v0.1454.0: pipes show what they are made of, and what they carry.**
  Every pipe used to be painted end to end in a made-up colour. Now copper
  looks like copper and hoses like rubber, and what a pipe carries is shown by
  marker bands that follow ISO 14726, the international standard for ships:
  at each end, past every bend, never more than 6 m apart. Settings has a
  Simplified mode (one band) and a Full mode (the standard's whole marker:
  drinking water blue, green, blue). Only water that has been through the
  purifier is marked as drinking water.

- **v0.1453.0: the trading post sells what it lists.** 140 of the trading
  post's 300 goods could never be bought or sold, because they were named
  differently from the real items (clay, cotton, flax, hemp, planks, bricks,
  nails, steel pipe and more). They now match, 26 real materials were added
  (ores, gems, ingots, cloth, plywood), and goods nothing in the game could use
  were taken off the list. A test now stops this from happening again.

- **v0.1452.2: three more Library guides about the ground.** Life in Soil
  (earthworms and what counting them tells you, tetanus and when a wound
  needs a doctor, manure and how long before harvest), Soil Chemistry (pH,
  lime and why it burns, reading a fertilizer bag, nitrate in well water and
  why boiling does not remove it), and The Ground Under You (rock and soil
  types, radon, sinkholes, calling 811 before you dig, and staying out of old
  mines). The Library now has 71 sourced guides.

- **v0.1452.1: three more Library guides, and the water pump tells the
  truth.** Moving Water Without Power (siphons, gravity lines, hand pumps,
  rain tanks and their weight, keeping children safe around water), Stone,
  Clay and Earth (dry stone walls, mud bricks, the right mortar for old and
  earth walls, silica dust and digging safely), and Fibre, Cord and Textiles
  (spinning and twisting cord, knots and how much they weaken a rope, moths,
  and clothes near a flame). The Library now has 68 sourced guides. The water
  pump's card said 12 litres a minute; it pumps 2, and now says so.

- **v0.1452.0: a day passes in 20 minutes on a shared server, carrying
  weight matters, and the real night sky.** A shared world's clock now runs
  at 72 times real speed by default, so a day passes in 20 minutes (the
  operator changed his mind the next night: from v0.1460.0 the default is
  real time, and acceleration waits for a later fast mode); the
  server's admin can change it from inside the app (Server Settings > Shared
  world clock), and the page says what a choice means, like how long a
  lettuce takes to grow. How much you can carry now follows gravity (about
  132 kg on Mars, none weightless), and in Realistic mode being over the
  limit slows your walk and stops you jumping. The stars are now where they
  really are for your place and date (Polaris due north, no southern stars
  from Washington), and they fade at dawn. The Donate page shows the two
  ways to give: the Sponsor-a-Can nonprofit (tax-deductible) or Patreon
  (direct to the maintainer, not tax-deductible). The homepage has a new
  night-to-sunrise clip over Mount Rainier from Silverdale.

- **v0.1451.0: walk out of your home and meet other players in the Commons.**
  In a shared world you can now walk from your own front door, down your
  corridor, into the ship's Commons, and see the other players there. Your
  neighbours' homes are drawn along the street with their own doors, which open
  for them. The game remembers your plot on each server, so your home is built
  in the right place before you even join. A guest on a full ship waits in the
  Commons. This is what a first two-person play session needs.

- **v0.1450.0: an erased account stays erased on every device, and server
  settings save safely.** After you erase your account on a server, the server
  now remembers for up to 30 days (a setting its admin can change) that it was
  erased, as a one-way fingerprint that is not your name or your data, so a
  second device that was switched off at the time cannot sign you up again by
  itself; pressing Connect is the only way back. The desktop Server Settings
  page no longer risks writing default values over a server's real settings.

- **v0.1449.2: three more Library guides.** Making an Agreement That Holds
  (putting a deal in writing, cosigning, sharing a well), Organising Shared
  Work (a work day with neighbours, lifting together, heat, and a review
  afterwards), and Teaching What You Know (showing someone a skill step by
  step, safely). The Library now has 65 sourced guides.

- **v0.1449.1: three more Library guides.** Chickens and Eggs (keeping a
  small flock, collecting and washing eggs safely, coop heat lamps), Glue and
  Joints (which glue for which job, and joints that hold), and Reading the
  Weather (clouds, pressure, watches and warnings, lightning, floods and
  tornadoes). The Library now has 62 sourced guides.

- **v0.1449.0: erasing your account sticks, and three more Library guides.**
  After you erase your account on a server, the app now disconnects from it and
  stays disconnected, instead of quietly signing you up again the next time it
  reconnects; the Chat page shows how to come back if you want to. New guides:
  Force, Levers and Mechanical Advantage; Heat and How It Moves; Rust, Rot and
  Decay (59 sourced guides). Behind the scenes, every test tool now checks it is
  testing exactly the current build, and none can start next to your own game.

- **v0.1448.0: every player's home has its own place on the mothership.**
  When you join a shared world, the server gives your home its own plot and
  you arrive at your own front door; a second player's home stands on the
  next plot, never on top of yours. Your built things, vehicles, animals and
  plants come with it. A server admin can free a plot from Server Settings,
  and erasing your account frees yours. This is the step before players can
  walk over and meet each other in the ship's Commons.

- **v0.1447.2: three more Library guides.** Sound and Hearing (how loud is
  too loud, and protecting your ears), What Not to Compost, Burn or Pour Away
  (batteries, medicines, paint, ashes, a broken thermometer), and Ratios and
  Mixing (formula, canning, concrete and mortar, and making water safe to
  drink with bleach). The Library now has 56 sourced guides.

- **v0.1447.1: three more Library guides.** Knowing Which Way Is North
  (compasses, declination, the shadow-stick and the stars, and what to do if
  lost), Coordinates (reading latitude, longitude and grid references, and how
  far off a phone can be), and Light, Lenses and Mirrors (magnifiers, glasses,
  mirrors, and keeping your eyes safe from the sun and lasers). The Library now
  has 53 sourced guides.

- **v0.1447.0: no more firewall pop-ups from development, and a choice of
  who can reach your node.** The test tools used to make Windows ask about the
  firewall many times a day; they now stay on the computer itself and never
  ask. "Host a node on this PC" gains a "Who can connect" choice: devices on
  your network (as before), or only this computer.

- **v0.1446.4: three more Library guides.** Nails, Screws and Bolts (which
  fastener for which job, and using a nail gun safely), Reading a Map
  (contours, scale and what to do the moment you think you are lost), and
  Reading Numbers Honestly (averages, ranges, and why one good year proves
  little). Each was checked against its sources first; the Library now has
  50 sourced guides.

- **v0.1446.3: three more Library guides.** Keeping Things Working (looking
  after tools, machines and a house, and making them safe before you start),
  Square, Level and Plumb (laying out a shed or a path, and calling 811 before
  you dig), and Trading Fairly (honest weights, recalls, contracts and scams).
  Each one was checked against its sources before it shipped.

- **v0.1446.2: three new Library guides.** Estimating (pacing out a
  distance, counting seconds to thunder, how much a drum of water weighs),
  Keeping Records (what a useful garden, rain, maintenance or money record
  holds, and how long to keep it), and Working Out Why Something Broke (a
  step-by-step way to find a fault, and where to stop and call someone). Each
  claim is backed by a public source such as the NWS, NIST, OSHA or the EPA,
  and an independent check read every source before they shipped.

- **v0.1446.0: the test tools know exactly which build they are testing
  (behind the scenes).** Every build now carries a fingerprint of the code it
  was made from, and the automatic checks refuse to test a build whose
  fingerprint does not match, so an old build can never pass for a new one.

- **v0.1445.0: round things are the right way out.** Every ball the game drew
  (light bulbs, the build-mode pipe beads, the planets in the hologram room,
  your avatar's head on its stand) was built inside out, so you saw its far
  inside instead of its outside. Fruit on plants was also lit from the inside.
  All fixed, with a test for every shape the game builds.

- **v0.1444.0: the test camera lands where it is told (behind the scenes).**
  The automatic picture-taking that checks the game after each change had been
  photographing empty space instead of the home, depending on what it looked
  at just before. It now lands on the spot it was asked for, and fails loudly
  when it does not, so a broken picture can no longer pass as a good one.

- **v0.1443.0: the greenhouse towers grow real plants.** The plants in the
  vertical towers used to show as black sprouts; they now show their real
  colours, lean out of their cups like real net-cup plants, and the beds and
  fields got brighter too. The cause was the plant colours being stored the
  wrong way, a mistake made when the models were converted.

- **v0.1442.0: your save keeps backups.** The game now keeps the last ten
  versions of your home's save, and Settings > Data can put any of them back.
  Other players' faces show properly (their hair used to cover them), and the
  crew stand on the floor with their heads on their shoulders. Under the hood,
  your home and the ship are now separate, the first step to every player
  having their own home on the mothership.

- **v0.1440.0: other players walk smoothly.** Another player's figure now walks
  at a steady pace instead of jerking forward in steps, keeps walking through a
  late update, and disappears at once when they leave. A scripted second player
  can now join to test with, so one person can see the shared world working.

- **v0.1438.0: the first quest takes either fuel.** Smelting your first iron
  ingot with graphite now counts, the same as with coal. The homepage also
  shows a real flight over a home in orbit, recorded in the game.

- **v0.1421.2: a server check found the self-hosted code mirror down (behind the scenes).**
  The website, game server and backups are all healthy. The project's own
  copy of the code at git.united-humanity.us had quietly stopped taking
  updates in early August; the tools now say so instead of staying silent.
- **v0.1422.0: sweating costs water.** Working or walking in the heat now
  uses up your water faster, the way it really does: an hour's walk in
  35C dry air sweats about a fifth of a litre.
- **v0.1422.4: the project's own copy of the code is back (behind the
  scenes).** git.united-humanity.us holds the full history again and now
  copies every change from GitHub by itself every few hours, so it can't
  quietly fall behind.
- **v0.1423.0: clearer words for your account.** The 24 words that back
  up your account are now called your "recovery phrase" everywhere (not
  the old crypto word, so it can't be mixed up with plant seeds), and the app
  says "no sign-up": you do have an account, it's just yours, on your own
  device.
- **v0.1424.0: everyone in a shared world shares one clock.** When you
  join the shared world, your time of day and date follow the host's, so
  everyone sees the same sun. Your garden keeps growing exactly as it was,
  and you can't sleep through a shared night.
- **v0.1428.0: humid heat is thirsty work.** Sweat that drips off
  instead of drying now costs water too, so a hot, humid day dehydrates
  you faster than a hot, dry one, as it really does.
- **v0.1431.0: other players look like themselves.** In a shared world,
  another player's figure now has their skin tone, their hair colour and
  their height, instead of every player looking the same.
- **v0.1433.0: trades hand over real items.** Offer something from your
  backpack, and when both of you confirm, it leaves your pack and what they
  offered arrives in yours, worn tools still worn. Before this, a finished
  trade said "items exchanged" and nothing moved.
- **v0.1435.0: the game can film itself.** A new recording mode turns
  set shots (Earth turning, Japan from orbit, a drop from 300 km toward Mt
  Fuji, the open sea, a sunset over the sea, Silverdale from above, the
  home from above, the greenhouse) into smooth, ready-to-post videos in
  both landscape and portrait.
- **v0.1437.0: trading works in the desktop app.** A finished trade now
  really moves the items, even if you were away when the other player
  confirmed; the Trade page lists your trades. Voice no longer drops you
  from the room after a network blip, and other players' figures stand on
  the floor with their head on their shoulders.
- **v0.1436.0: the Windows download is the whole game.** The main
  download button now gives Windows players the full game with its models
  and textures; before, it gave them the bare program, which looked like grey
  boxes. Upvoting a bug and the Tasks board on the website work again too.
- **v0.1435.2: chat works on older phones.** The website chat refused
  to connect on phones whose browser is not the newest (anything before
  Chrome 137), which is most phones in much of the world. A user in
  Nigeria reported it; it is fixed, and his identity will stay the same
  when his browser updates.
- **v0.1434.0: the ship's districts look like what they are.** The mall
  has market stalls, the hangars have cradles for ships, and the power and
  industrial decks have rows of machines with pipes and ducts, instead of
  plain blocks.
