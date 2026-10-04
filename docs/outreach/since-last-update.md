# Since the last public update

A running list of what has shipped since the last posts went out, kept so the
next update can be written from it without digging through the logs. The last
posts were the evening update of 28 September 2026
(`docs/outreach/posts/2026-09-28-evening-update.md`, v0.1396.1 to v0.1421.0).
When the next update is written, move this list into it and start a fresh one.

Each line is written the way a player would hear it; the release notes and
`docs/history/` hold the detail.

## Shipped

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
  at 72 times real speed by default, so a day passes in 20 minutes; the
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
