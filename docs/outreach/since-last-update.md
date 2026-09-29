# Since the last public update

A running list of what has shipped since the last posts went out, kept so the
next update can be written from it without digging through the logs. The last
posts were the week in review for 21 to 27 September 2026
(`docs/outreach/posts/2026-09-28-week-in-review.md`). When the next update is
written, move this list into it and start a fresh one.

Each line is written the way a player would hear it; the release notes and
`docs/history/` hold the detail.

## Shipped (28 September 2026)

- **v0.1396.1: two food guides checked twice, and jerky can be made again.**
  Drying Food and Fermenting Vegetables passed their independent second pass
  (18 of 143 curriculum topics are now verified). Make Jerky needed dried meat
  and Dry Meat needed jerky, so neither could ever start; both now start from
  mutton, which sheep and goats give.
- **v0.1397.0: the home keeps its own time.** The home station now stays over
  its spot above Earth all year (it used to drift a full turn over a year), and
  its solar panels, grow lights, crops, HUD clock and wake-up note all follow
  the home's own local time, so noon on the HUD is noon on the deck.
- **v0.1398.0: no more flat rings on dark skies.** Night skies, the glow at the
  planet's edge, twilight, fog and the aurora used to draw in flat bands with
  hard edges. The scene now renders at high precision and gets one fine grain
  at the end. Measured: the share of dark pixels in long flat bands fell from
  53% to 10% at the darkest aurora view, 70% to 27% at a night horizon, 54% to
  9% at dawn on the shore; cost about 0.05 ms a frame.
- **v0.1399.0: put your back to the wind.** A shelter keeps the wind off only
  when its open side faces away from the wind, as survival manuals teach; the
  HUD says how much of the wind gets in.
- **v0.1400.0: what you build is solid aboard.** Walls, beds, chests and
  machines stop you like the ship's own walls.
- **v0.1401.0: solid on planets too, and you can take things down.** Built
  pieces are solid on a planet's ground as well. With a piece in hand, F takes
  down what you are looking at and gives all its materials back; a chest with
  anything in it stays up until you empty it.
- **v0.1402.0: doors in walls.** Build a Wood Wall with Door and press E
  to open or shut it; an open door lets you walk through, a shut one stops you.
  A closed room finally has a way in and out.
- **v0.1403.0: doors remember.** A door you leave open is still open
  when you load your save.
- **v0.1404.0: windows in walls.** A Wood Wall with Window (a pane of
  glass you smelt from sand) lets you see out and is solid to walk into.
- **v0.1405.0: building fixes from a review.** You can now aim through an
  open doorway at what is beyond it, a piece still going up blocks taking down
  what stands behind it, a roof on the ship no longer reads open to a wind that
  is not there, and the old free-standing door and window pieces (which built
  solid slabs you could not open) are gone.
- **v0.1406.0: no reaching through walls.** Aboard, E and F no longer reach a
  chest or a bed in the next room through the home's walls.
- **v0.1407.0: walls meet the roof on uneven ground.** Walls built
  straight on bumpy ground now stretch or shrink a little so their tops line
  up, and the roof sits flush with no line of sky under it.
- **v0.1408.0: the sea follows the wind where you are.** The waves used to
  follow the weather's wind for the whole planet; now they follow the wind at
  your spot, the same wind your body feels.
- **v0.1409.0: the waves' shape follows too.** The wave shapes now follow the
  same local wind as the sea's colour and shine, so the two always agree.
- **v0.1410.0: the take-down message reads right.** Taking a piece down now
  says what came back by name ("6 Wood Plank") instead of an item code.
- **v0.1411.0: doors you build make a sound.** Opening and shutting a built
  door plays the same sounds as the ship's own doors.
