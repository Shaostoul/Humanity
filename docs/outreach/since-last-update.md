# Since the last public update

A running list of what has shipped since the last posts went out, kept so the
next update can be written from it without digging through the logs. The last
posts were the evening update of 28 September 2026
(`docs/outreach/posts/2026-09-28-evening-update.md`, v0.1396.1 to v0.1421.0).
When the next update is written, move this list into it and start a fresh one.

Each line is written the way a player would hear it; the release notes and
`docs/history/` hold the detail.

## Shipped

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
