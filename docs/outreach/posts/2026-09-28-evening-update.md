# Update: 28 September 2026, evening

Posts for X (a thread), Discord and Facebook, covering everything since the
week in review (`2026-09-28-week-in-review.md`): v0.1396.1 to v0.1421.0, 29
releases on GitHub. Every claim was checked against the release notes and
`docs/history/2026-09-28.md`. The "feels about 5C" figure is the body heat
model's operative temperature for a still, clear 10 C night (the sky's
radiant temperature alone is about 0.5 C, which is why the running list's
"close to freezing" was not used). The "feels 30C" line is a real rig capture
(`planet-open-noon-walk`).

## X thread

1/ Today in HumanityOS: 29 releases. It's a free, open-source platform with one goal: end poverty and unite humanity. The game teaches real survival skills by simulating them honestly. Here's what's new. 🧵

2/ Building grew up. Walls with doors you open and shut (they remember, and they make a sound), walls with glass windows, pieces that are solid to walk into, and F to take a piece down and get every material back. Walls on bumpy ground now meet the roof flush.

3/ Shelter works the way survival manuals say: put your back to the wind. On a clear night your body loses heat to the cold sky, so 10C in the open feels about 5C, and a roof keeps that off. In full sun, a roof is shade.

4/ The sun is real on your skin now, using ASHRAE's SolarCal method. The weather line says what the air feels like: on Sahara sand at noon, 22C air in a 4 m/s wind read "feels 30C". And no more false "Air" warning while you're breathing Earth's air.

5/ Time and power: the ship keeps its own local time all year, so noon on the HUD is noon on deck. Solar panels follow the sun where they stand, and a panel on the ground makes about a quarter of its power under full overcast, like a real one.

6/ The sea's waves now follow the wind where you are, not the planet's average. And night skies, twilight and the aurora lost their flat bands: dark gradients are smooth now.

7/ Multiplayer groundwork: the server now understands the same save files as the game, and other players' names float over them. Plus jerky can be made again (its recipe went in a circle), and two more Library food guides passed their second, independent check.

8/ Everything is free and open source.
united-humanity.us
github.com/Shaostoul/Humanity

## Discord

**HumanityOS: 28 September, evening update** 🛠️

29 releases since the week in review. The theme was **shelter that works like real shelter**, and the sun, the sky and the wind now reach your body the way they really do.

**Building**
- **Doors in walls:** build a Wood Wall with Door and press E to open or shut it. Open lets you through, shut stops you. Doors remember across a save and make the same sounds as the ship's doors.
- **Windows in walls:** a Wood Wall with Window, glass you smelt from sand. You see out, you can't walk through.
- **Solid:** what you build stops you, aboard and on a planet's ground.
- **Take it down:** with a piece in hand, F takes down what you look at and gives every material back (a chest with things in it stays up until you empty it).
- Walls built straight on bumpy ground stretch or shrink a little so the roof sits flush.

**Your body and shelter**
- Put your back to the wind: a shelter keeps the wind off only when its open side faces away from it, and the HUD says how much gets in.
- The clear night sky pulls heat from your body: a still, clear 10C night feels about 5C in the open, and a roof overhead keeps that off.
- The sun warms you the way it really does (ASHRAE 55's SolarCal), and a roof is shade on a hot day.
- The weather line adds what the air **feels** like when it differs from the thermometer: "Clear 22C  wind 4 m/s from N  feels 30C" on Sahara sand at noon.
- No more yellow "Air" warning while you breathe Earth's air.

**Time, power and weather**
- The ship stays over its spot above Earth all year, and its panels, grow lights, crops and HUD clock follow its own local time.
- A panel built on a planet follows that place's sun, and clouds cut it to about a quarter under full overcast.
- On the Moon and Mars, the day's heat comes at your own afternoon.
- The sea's colour, shine and wave shapes follow the wind where you are.

**The view**
- Dark skies, twilight, fog and the aurora no longer draw in flat bands. At a night horizon the share of dark pixels in long flat bands fell from 70% to 27%, for about 0.05 ms a frame.

**Multiplayer groundwork**
- The server now reads and writes the same save format as the game.
- Other players' names float over them, like the crew's.

**Recipes and Library** 📚
- Jerky and dried meat each needed the other, so neither could be made; both now start from mutton.
- Drying Food and Fermenting Vegetables passed their independent second check (18 of 143 curriculum topics verified).

Free and open source: https://united-humanity.us · https://github.com/Shaostoul/Humanity

## Facebook

An evening of HumanityOS 🏡

HumanityOS is a free, open-source project with a big goal: end poverty and bring people together. Part of it is a game that teaches real survival skills, like building shelter, staying warm and growing food, by simulating them honestly.

Today we shipped 29 updates. Here are the highlights.

🚪 Real walls. You can build walls with doors that open and shut, and walls with glass windows. What you build is solid now, and if you change your mind you can take a piece down and get all your materials back.

🌙 Shelter that works like real shelter. Survival guides say to put your back to the wind, and now that matters. On a clear night your body loses heat to the open sky, so 10 degrees can feel like 5, and a roof over your head keeps that off. On a sunny day, a roof gives you shade.

☀️ The sun on your skin. Standing in full sun now warms you the way it really does, and the weather line tells you what it feels like: on the desert at noon, 22 degrees felt like 30.

🔋 Power and time. The ship keeps its own local time, solar panels follow the sun where they stand, and cloudy days mean less solar power, just like at home.

🌊 The sea and sky. Waves follow the wind where you are, and night skies and sunsets are smooth instead of banded.

🤝 Playing together. Behind the scenes, the server now understands the game's save files, and you can see other players' names above them.

📚 Two more food guides in the free Library were checked a second time against their sources, and a recipe loop that stopped anyone making jerky is fixed.

Everything is free and open source. 👉 https://united-humanity.us

## What shipped (the list these were written from)

Moved here from `docs/outreach/since-last-update.md`, which starts fresh.

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
- **v0.1403.0: doors remember.** A door you leave open is still open
  when you load your save.
- **v0.1404.0: windows in walls.** A Wood Wall with Window (a pane of
  glass you smelt from sand) lets you see out and is solid to walk into.
- **v0.1405.0: building fixes from a review.** Aim through an open doorway, a
  piece still going up blocks taking down what stands behind it, a roof on the
  ship no longer reads open to a wind that is not there, and the old
  free-standing door and window pieces are gone.
- **v0.1406.0: no reaching through walls.** Aboard, E and F no longer reach a
  chest or a bed in the next room through the home's walls.
- **v0.1407.0: walls meet the roof on uneven ground.**
- **v0.1408.0 and v0.1409.0: the sea follows the wind where you are**, its
  colour and shine first, then its wave shapes.
- **v0.1410.0: the take-down message reads right** ("6 Wood Plank").
- **v0.1411.0: doors you build make a sound.**
- **v0.1412.0: the HUD says the air you are in** aboard ("Indoors 20C, still
  air"), and the clock's night icon is a moon again.
- **v0.1413.0: a solar panel sees the sun where it stands.**
- **v0.1414.0: the day's heat comes at your own afternoon** on the Moon or
  Mars.
- **v0.1415.0: first step toward a shared world (behind the scenes):** the
  server reads and writes the game's save format.
- **v0.1416.0: a clear night feels cold, and a roof helps.**
- **v0.1417.0: the sun warms you, and shade matters.**
- **v0.1418.0: cloudy days mean less solar power** on the ground.
- **v0.1419.0: the weather line says what it feels like.**
- **v0.1420.0: no false alarm about your air on Earth.**
- **v0.1421.0: you can see who other players are.**

Also on GitHub in this span: v0.1410.1 and v0.1412.1 (docs only).
