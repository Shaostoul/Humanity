# The game's setting and its intro: why the fleet is between the stars, and how you arrive

**Status: a design, written 2026-11-02 against main at 5aefdb02f. Nothing in it is built.** The
operator decides. Every recommendation here is the author's (an agent's) unless it is marked as the
orchestrator's or as the operator's own words. Section 10 holds the questions only he can answer,
each with a recommendation.

**How claims are marked** (the convention of [ship-homes-and-logistics.md](ship-homes-and-logistics.md)
and [leaving-the-ship.md](leaving-the-ship.md)):
- Repo facts cite a file, and a line where it helps, read at 5aefdb02f. Lines drift.
- World facts cite a source and the source's own date.
- "Computed" means arithmetic on cited figures; Appendix B shows the working.
- "Assumption" means a number of ours that data or a measurement should replace.
- Draft words (what a person would read) are marked as drafts. Words people read come from
  `data/i18n/en.json` by key when built; this design writes no UI.

---

## 0. In short

- **The setting.** Centuries from now humanity is one people, at peace. Earth is healthy; nobody had
  to leave. The fleet left by choice, to see what is out there and to learn to live well anywhere.
  Its motherships cruise between the stars under a drive that never stops, and every home aboard is
  a working Earth homestead: Earth's weight at the drum's rim, Earth's day, Earth's crops in real
  soil. That makes the fleet the largest experiment in living well people have ever run, and the
  best designs go two ways: ahead to the worlds the fleet will settle, and home to Earth. That is the
  tie the operator asked for between interstellar space and "designing the best Earth homesteads
  possible" (section 1).
- **Coming back as yourself: one mind on two supports, never copied** (the orchestrator's answer,
  adopted). Three rules a player can feel: the first-person feed never cuts (death dims it); there
  are never two of you; the fleet never wakes a copy. The third rule is what makes a final death
  possible without banning anyone (section 2).
- **The link has an edge:** 1 light-year around each mothership to start, the operator's number.
  The jump (if he adopts FTL, question 1) reaches 1 light-year to start too, so at first nothing a
  new player can reach is beyond the link. The fleet's upgrades are four separate lines (link range,
  jump reach, cruise speed, point defense), chosen by the crew's vote and built from their
  contributions. When the jump reach grows past the link, the band between them is the frontier,
  and beacons that players build pull the link out into it (section 3).
- **Beyond the link, a recorder keeps you.** Simplified and Realistic issue one to everyone and it
  cannot fail; Hardcore makes it real gear that can. Nobody is banned for dying: a Hardcore
  character can end, the person never does, and they arrive again as a new crew member on the next
  transport (section 4, the table in 4.2).
- **The intro:** a ride in a transport's window seat with a dozen other arrivals and a hold of cargo,
  the ship loading behind the view; stepping off into the hangar as the cargo comes out; the
  Arrivals desk (a bedroom of your own, where you live, how hard the world pushes back, where you
  wake); general services for anyone who walks past the desk; the walk home into the opening quests
  that already exist. Skippable after the first time (section 5).
- **Multiplayer:** the ride is client-local. The server holds only a seat on a scheduled trip, a
  few hundred bytes per arrival and nothing per frame; the same shape serves a dozen friends, an MMO
  and a solo game (section 6).
- **Nine rungs, each with its gate;** the first seven need no FTL decision (section 9).
- **Thirteen questions for the operator** (section 10).

## 0.1 The operator's words (quoted exactly)

**(a) The question, 2026-11-02:**

> I feel like we need to work on an intro for the game. So people understand the setting of the game being set in interstellar space a lot of the time. And how that ties into designing the best Earth homesteads possible. Remember we're going from the approach Humanity is united in peaceful harmony. Players are the individual crew members of the fleet. They live their immortal lives, Contribute to the fleet in whatever way they enjoy/can. Essentially resurrect like the Cylons from Battlestar Galactica. They maintain all of their knowledge but, lost everything physical, including any body mods/changes. I haven't figured out how we justify the respawning mechanic yet. Theoretically there's the soul, mind, and body. Then people talk about source energy. I'm just not sure how to justify that the person that resurrects is actually the person that died on a game mission. It's the teleportation paradox. Is the person that goes through the teleport still you? Yes, it can be a perfect replica but, did you (well me since I'm human) cease to exist. Like does the first person camera feed ends for me permanently. Any ideas how we could justify resurrecting people that remain themselves?

**(c) His reply to the orchestrator's line about range, the same day:**

> I guess that's a feature we could add. The motherships broadcast X range from the ship meaning we could only jump 1 light year out or something like that. The players could upgrade the fleet to jump further, fly faster, have better guns, etc. How would death out there be final? How would we enforce that? The player still wants to play on the server. Do we really want to ban people? What are our other options?

**(e) His intro, the same day:**

> The intro of Half-Life 1 has really stuck with me but, I wanted to take it to the next level. I was thinking the intro could actually be arriving as a new crew member to the main colony ship. They could be sitting at a window seat or like a front passenger seat is it's a small spacecraft like the Star Trek runabout ships. Flying in from space on a transport spacecraft with other people and cargo. Exiting the craft they see the cargo being unloaded and what not. A single cargo ship could hold as many players as we want, like a doze or more so that many people can join a new server at once without spawning 12 transport ships Start the player off with a bedroom of their own. Then upon leaving the ship they check in with admin guy like a receptionist and choose their play style (simple home, advanced, or whatever). If they never check in and something happens to them they go to general services like a barracks and medbay. With a proper home players could respawn at their home or choose where they want to respawn.

**What the orchestrator answered, the same day. These are recommendations, never his decisions:**

- (b), to (a): one mind on two supports, never copied; every crew member's mind runs on their brain
  AND on the fleet's network at once, all the time, "the way the brain's two halves are one person",
  not a backup taken now and then. Death dims the feed and does not end it; the respawner grows a new
  body, the mind settles in, and you wake. It is still you because identity is the pattern that keeps
  running, not the matter. Knowledge stays because it is the mind; everything physical is lost; body
  mods are gone because the new body is grown from the genes you were born with; each new body costs
  the fleet matter and energy (the used/contributed ledger). And the line he replied to: "The link
  needs the fleet's relays, so missions out of range are the real danger: a hardcore setting where
  death out there is final, and a softened one where you come back."
- (d), to (c): four options, of which option 1 was wrong (it woke the player "from the last backup"
  taken on leaving range, which is a copy waking, the very paradox (b) removes). The consistent
  version, which this design uses and extends in section 4: nobody is banned for dying; out of range
  the mind has only the brain unless it carries a RECORDER, a portable second support; with one, a
  death out there is still sleep until the recorder comes back into range; Hardcore, chosen per
  character at check-in, ends a character whose recorder is destroyed or never comes home in its
  power life, and the person arrives again as a new crew member; Simplified, the default, brings the
  recorder home by itself and costs what a Realistic death costs now.
- On (e): the ride can hide the loading; checking in is where the mechanic is explained, so the
  story teaches it without a tutorial; the ride is skippable after the first time.

**Earlier decisions this builds on (his words where quoted):**

- **The fleet's usual state, 2026-09-15** ([gravity-and-movement.md](gravity-and-movement.md)): "The
  most common state of the fleet is going to be in the perpetually forward motion going between stars
  so it makes sense the spine has gravity."
- **No FTL, decided 2026-09-15; reopened by him 2026-10-05 as a proposal** (the same document): "What
  do you think about having FTL but, designing the ships to survive indefinitely in case the FTL
  engines become permanently unavailable? Y'know, just incase the apocalypse happens... again." The
  orchestrator recommended adopting it; it is awaiting his answer (docs/PRIORITIES.md, "FTL
  reopened"). His 2026-11-02 words "jump 1 light year out" read as that direction, but he has not
  said so: question 1.
- **The mothership, 2026-09-19** ([the-mothership.md](the-mothership.md)): "the homeship is meant to be
  premade. Kind of like a giant flying lived in city", and "The player just happens to be one of the
  crew members."
- **The starts, 2026-10-03** ([ship-homes-and-logistics.md](ship-homes-and-logistics.md) section 1):
  Cabin, Apartment and Homestead (the full list of home kinds as data), "the default full homestead
  with everything".
- **Arriving on a server, 2026-10-04** (docs/history/journal-archive-2026-10.md, the answers to
  ship-homes section 9): a server home is fresh; one mothership gateway, which at first only lets
  players arrive; and, quoted, "only their appearance is kept. Any upgrades, cybernetics, genetic
  modifications, etc. would be removed upon entering the server world so they're fresh."
- **Real time, 2026-10-04** ([first-hour-audit-2026-10-04.md](first-hour-audit-2026-10-04.md)): "For
  normal mode, especially for my MMO server, let's have everything be real time, not the 72x."
- **Death by mode, 2026-10-04** ([death-and-your-pack.md](death-and-your-pack.md)): Simplified loses
  nothing; in Realistic "your carried items would stay where you fell for a while, to go back for."
- **The fleet's stores, 2026-10-04** (data/ship/fleet_ledger.ron): "we'll say the fleet has unlimited
  of everything and just track what they player uses and contributes."
- **The two realities, 2026-07-16** ([two-realities.md](two-realities.md)): the app is your real life,
  and the game is a door you walk through (Play), not a toggle.
- **A dozen at least, 2026-09-15** ([game-modes.md](game-modes.md) F): "whatever co-op game mode we
  make has to support at least a dozen."

---

## 1. The setting

### 1.1 The page a new player could read

A draft for a Library page and for the words the game shows on first arrival. Final words go into
the Library and `data/i18n/en.json` by key when built.

> **The Fleet**
>
> It is centuries from now, and humanity is one people.
>
> We did not get here by beating each other. What kept people poor and afraid was never other
> people. It was pollution, poverty, corruption, fraud and tyranny, and we ended them the way you fix
> a machine: by building ways of living in which they stopped paying. People still disagree. We
> settle it by talking and by voting, not by fighting. Peaceful harmony is not the absence of
> conflict.
>
> Earth is healthy. Nobody had to leave. We left anyway, the way people always have: to see what is
> out there, and to learn to live well anywhere.
>
> The fleet is that leaving. Its motherships are cities flying between the stars, each one home to
> more people than most countries, with transports, freighters and scouts running between them. The
> drives never stop. They push the fleet forward day and night, and that push is the weight you feel
> in a ship's spine. The great drums turn, and at their rims you weigh what you weighed on Earth.
>
> Between the stars there is no sun, no soil and no shop. Everything a person needs, the fleet makes
> for itself: air, water, food, power, tools and homes. That makes the fleet the biggest experiment
> in living well that people have ever run. Every home aboard is a real home: an Earth homestead
> under Earth's weight, on Earth's day, growing Earth's crops in real soil, with real water, real
> power and real repairs. What works is kept. What fails is fixed. The best of it goes two ways:
> ahead, to the worlds the fleet will settle, and back home to Earth, where people build the same
> designs on the same kind of land.
>
> So your home matters. What you learn keeping it alive is not made up. The plants, the numbers and
> the skills are real, and you can use them where you really live.
>
> You are crew, and nobody aboard goes without. Help in whatever way you enjoy: grow food, build,
> fix, haul, cook, teach, explore, look after people. There is no wrong way to help.
>
> Crew live long. When you come aboard you are joined to the link, and from then on your mind runs
> on your brain and on the link together. If your body dies, you do not: you fall asleep, and you
> wake in a new body grown from the genes you were born with. You keep everything you know. You lose
> everything physical: what you carried, and anything that was added to your body.
>
> The link has an edge. Past it, carry a recorder, and go with friends.
>
> Today you arrive. A transport is bringing you in with a dozen other new crew and a hold full of
> cargo. Welcome aboard.

### 1.2 Why each line is there (for whoever builds it)

- **One people, at peace.** (a): "Humanity is united in peaceful harmony". The five things that were
  ended are the project's own five adversaries, defined as structures, not people
  ([docs/mission/the_five_adversaries.md](../mission/the_five_adversaries.md)), whose closing line
  the page quotes: "Peaceful harmony is not the absence of conflict." Conflict between people is
  settled by governance (the signed votes already live, docs/FEATURES.md "Native Governance page"),
  never by weapons. What weapons are for is section 3.5.
- **Earth is healthy; we left by choice.** The older lore says the same:
  [docs/game/humanity_one.md](../game/humanity_one.md) ("The game takes place well after departure,
  the ship is established, lived-in, home") and the subtext of
  [docs/game/intro_sequence.md](../game/intro_sequence.md) ("No apocalypse backstory, no trauma
  motivation, just curiosity, ambition, and the deep human need to explore and create"). This design
  keeps that subtext and replaces that document's intro (Appendix C).
- **Between the stars, under a drive that never stops.** His 2026-09-15 words above. The spine's
  weight is the drive's thrust and the drum's is spin
  ([gravity-and-movement.md](gravity-and-movement.md); the hyperboloid table there gives 1.00 g at
  the drum's flared ends).
- **No sun between the stars.** [closed-loop-test.md](closed-loop-test.md) already says every ship
  farm out there is lit by lamps. The game does not meet this yet: today the ship hangs over
  Silverdale in a synchronous orbit (data/stations/home.ron:33-45) and First Street is lit through
  its glass roof (data/blueprints/ship_structure.ron, header). Where the fleet is becomes a property
  of each world (question 2), and the interior lighting that does not need the Sun is a rung
  (section 9, rung 7).
- **The experiment, and the two directions.** This is the tie (a) asked for. "Ahead" is the fleet's
  purpose. "Home" is honest either way: if the link reaches Earth through the beacons the fleet left
  behind it (question 3), designs arrive at once; if not, they travel at light speed and arrive years
  later, as signed bundles ([interplanetary.md](interplanetary.md)). Either way the page's sentence
  stays true.
- **Real for you.** The person playing is a person on Earth. [two-realities.md](two-realities.md):
  "we teach real-life skills by mirroring them in the game." The game is a door (Play); the app
  around it is the person's real life, where the real side of the same skill is tracked. (The brief
  and CLAUDE.md call this the Real/Sim toggle; two-realities.md removed the toggle on 2026-07-16.
  Appendix C.)
- **Crew, and nobody goes without.** (a): "Contribute to the fleet in whatever way they enjoy/can."
  the-mothership.md: the ship runs without any one player. The fleet ledger counts what each person
  used and gave without ever refusing anyone (data/ship/fleet_ledger.ron); a new body is one more
  line in it (section 2.4).
- **The link and its edge.** Sections 2 to 4.
- **The film's canon.** The festival film's script
  ([docs/outreach/film/intro-film-script.md](../outreach/film/intro-film-script.md), v2 of
  2026-08-15) proposes story canon for a game story bible: humanity unified before going
  interstellar; the First Fleet; the expansion loop "arrive at a system, establish a permanent
  colony, light its beacon, reach for the next system"; and the main story arc's destination
  mystery, the artificial world Agartha ([artificial-planet.md](artificial-planet.md)). This setting
  agrees with all of it and borrows the film's beacon for the thing players build to extend the link
  (section 3.6). It decides nothing about the arc. It does give the arc one useful fact: the real
  danger of any story lives beyond the link, where death can be final.
- **The soul.** (a) names soul, mind, body and source energy. The page explains only the mind's
  running and says nothing about the soul. Recommended for the Library page, a draft line: "The link
  carries your mind. What else a person is, the fleet leaves to each person." A game for billions of
  people should not claim to settle that question, and nothing in the mechanics needs it to.

---

## 2. Coming back as yourself

### 2.1 The rule: one mind on two supports, never copied

The orchestrator's answer (b), adopted whole. From the moment a person comes aboard, their mind runs
on their brain AND on the link at the same time, every moment. It is not a backup taken now and
then. When the body dies the mind loses one support and keeps running on the other; a respawner
grows a new body; the mind settles into it; the person wakes.

Three rules make it something a player can feel, and each is a rule the code can hold:

1. **Never a gap.** The first-person feed never ends. At death it dims, like the edge of sleep, and
   it brightens again on waking. That is the direct answer to his "does the first person camera feed
   ends for me permanently": no, because nothing stopped.
2. **Never two.** A new body is grown only after the old one has died, and only one body carries a
   mind at a time. Nothing in the game makes a second of you.
3. **Never a copy.** The fleet never starts a mind from a stored state. When a person leaves the
   link's reach, the link lets go of them and keeps nothing it could wake. If a mind ever has no
   support left, that person has died, and waking a stored copy would be making someone new and
   giving them a dead person's name. The fleet does not do it.

The teleporter's problem is a feed that ends here while another begins there. Under these rules no
feed ever begins, because none ever ended. And rule 3 is what makes a final death possible
without any ban (section 4.3): there is nothing to restore.

### 2.2 How a player meets it

Three places, in the order a new player reaches them. All words are drafts.

1. **The Arrivals desk** (section 5.4), the receptionist: "You were joined to the link when you
   boarded. From now on your mind runs on your brain and on the link together. If your body dies,
   you won't: it'll feel like falling asleep, and you'll wake in a respawner. Let's choose which
   one."
2. **The first death.** Today the death screen says "YOU DIED", the cause and a Respawn button
   (src/gui/pages/hud.rs `draw_death_screen`). Drafted for the lore: "Your body died. You did not."
   / "You are asleep on the link." / where you will wake, and in Realistic what stayed where you
   fell (the existing lines) / a button to wake.
3. **A Library page, "Coming back as yourself":**

> People have always asked the teleporter question. If a machine takes you apart here and builds a
> perfect copy over there, did you travel, or did you die while someone new woke up? The copy would
> say it was you. But your own view of the world would have ended.
>
> The fleet never does that. It never copies anyone.
>
> When you joined the link, your mind began to run in two places at once: on your brain, and on the
> link. Not a backup taken now and then: both at once, every moment, the way the two halves of your
> brain carry one person now. Children who lose half their brain to surgery grow up as themselves.
> One support was enough to carry them.
>
> So when your body dies, nothing stops. Your mind loses one support and keeps running on the other.
> It feels like the edge of sleep. The respawner grows a new body from the genes you were born with,
> your mind settles into it, and you wake. You remember falling asleep. You remember everything.
>
> It is still you for a plain reason: you are the pattern that keeps running, not the stuff it runs
> on. The molecules in your brain are replaced again and again through your life, and you stay you.
>
> Three rules keep it honest. Your view of the world never cuts out. There are never two of you. And
> the fleet never wakes a copy: if a mind ever has no support left, that person has died, and the
> fleet will not make someone new and give them your name.
>
> That last rule is why the link's edge matters. Beyond it, your mind has only your brain, unless
> you carry a recorder.
>
> The link carries your mind. What else a person is, the fleet leaves to each person.

### 2.3 Why it is still you: the real ideas underneath

The lore is fiction (section 2.5), but the argument it makes is real philosophy and real biology,
and a teaching game should be able to show its working.

- **His teleporter question is Derek Parfit's.** *Reasons and Persons* (Oxford, 1984), chapter 10,
  "Simple Teletransportation and the Branch-Line Case": in the branch line the scanner fails, the
  original is left on Earth, fatally harmed, while a replica lives on Mars. The two cases this
  design forbids, a gap and a second, are exactly the two that make his question hard.
- **Gradual transfer.** Hans Moravec, *Mind Children* (Harvard, 1988), chapter 4: a robot surgeon
  replaces a brain neuron by neuron while the patient stays awake, so the person who gets up is the
  one who lay down. The link is the same intuition without the operation: both supports run all
  along, so there is no moment of transfer at all.
- **The ship of Theseus.** Plutarch's old question (*Life of Theseus*): a ship rebuilt plank by plank
  over the years is still the ship.
- **Matter changes, the person does not.** Brain proteins in mice last about 9 days on average:
  Price, Guan, Burlingame, Prusiner and Ghaemmaghami, "Analysis of proteome dynamics in the mouse
  brain", *PNAS* 107(32):14508-13, 2010-08-10 (1,010 proteins measured). To be exact about it: the
  cells themselves mostly last a lifetime. Neurons of the human occipital cortex are as old as the
  person (Spalding and others, "Retrospective birth dating of cells in humans", *Cell*, 2005-07-15).
  So the page says the brain's molecules are replaced, never that the brain is.
- **One support is enough.** People who had one cerebral hemisphere removed in childhood, to stop
  severe epilepsy, grow up as themselves. Kliemann and others, "Intrinsic Functional Connectivity of
  the Brain in Adults with a Single Cerebral Hemisphere", *Cell Reports* 29:2398-2407, 2019-11-19,
  scanned six such adults and found the networks within their remaining half as strongly connected
  as in people with two. The link is the made-up part; losing one support and staying yourself is not.
- **The honest caveat.** Philosophers still disagree, and Parfit himself argued that identity is not
  what matters. The game does not need to win the argument. It needs one rule a player can feel:
  the camera never cuts.

### 2.4 The rules this implies

| What | The rule | Today (code) | New |
|---|---|---|---|
| Knowledge: skills, quests, recipes, what was learned | Kept: it is the mind. | Kept through death (death-and-your-pack.md, "What is kept") | nothing |
| What you carried | Simplified: the fleet brings it back with you. Realistic and Hardcore: one pack where you fell, 60 minutes of play to fetch it. | Exactly this (crates/humanity-systems/src/death_pack.rs, data/world/death.ron) | the lore's words for it |
| What you wore and had equipped | Re-issued to you from the fleet's stores, as crew kit, so you never wake with nothing. | Kept on you in Realistic (death-and-your-pack.md); the operator accepted that rule | a ledger line for the re-issue; the lore's words |
| Your look | Kept. The fleet grows you from your genes and the look you keep at the mirror. This matches his arrival rule of 2026-10-04: "only their appearance is kept". | The look (name, appearance, outfit) already crosses into every world (src/engine/save_slot.rs `Look`) | nothing |
| Body mods: implants, augments, cybernetics, genetic modifications (his 2026-10-04 list), and wounds, illness, scars, conditioning | Gone. They were added to the body, not born in the genes, so a new body does not have them. A mod can be fitted again at its own cost. | No body-mod system exists. Respawn already restores health, raises vitals to at least 60 and clears status effects (src/lib.rs, the `pending_respawn` block) | the rule, when mods exist |
| The body itself | One "body" line in the fleet ledger, Used, at a named placeholder price of 0 until body materials exist as goods (the pattern leaving-the-ship.md uses for a seat). Nobody is ever refused a body, in the red or in the black (question 8). | The ledger exists (data/ship/fleet_ledger.ron: meal, item, power) | a ledger kind `body` (and `reissue`) |
| Where you wake | Your home's respawner by default; general services with no home; any respawner you are allowed to use (section 5.6). | You stand at your own front door; a guest at the Commons (src/engine/home_plot.rs, `fps_spawn`); through the relay in a shared world (`respawn_through_relay`) | respawners as places; a chosen wake place in the save |
| How long it takes inside the link | At once, in every mode (recommended, question 9). The cost of a death is what it loses, never a wait for its own sake. | At once (the Respawn button) | nothing |
| Two of you | Impossible: a body is grown only for a mind whose body has died. | Respawn exists only on the death screen | a server check once respawners are shared places |

### 2.5 What is made up, said plainly

A teaching game must never teach a fiction as fact. Four things here are made up, and the Library
should say so on one page, "What is made up in this game", in the spirit of the operator's
2026-09-18 rule about saying plainly what we will not build:

1. A mind can run on a machine as well as on a brain. Nobody knows how, or whether it can be done.
2. The link reaches further than light could carry it (section 3.1).
3. The jump, if he adopts it (question 1).
4. A respawner grows an adult body in moments. Real bodies take about two decades.

Everything else (the crops, the water, the air, the power, the gravity of a spinning drum, the
distances between the stars, the dust that hits like a bomb at the fleet's speed) is as real as the
project can make it.

---

## 3. The link's range

### 3.1 Why the link has to be made up past a few thousand kilometres

("The link" is this design's working name, as in the brief. What people read is question 11,
because "link" already means a web address on the app's screens.)

Two supports of one mind have to agree quickly. Assumption: within about 10 ms, the order of time
a signal takes between the brain's two halves (an assumption, not a measured limit for a mind).
Light covers 2,998 km in 10 ms, and the Moon is 1.28 light-seconds away (computed). So a link that
obeyed the speed of light would end before the Moon. His light-year range is only possible as a
fiction, and the design should say so once (2.5) and then treat it as a rule of the world.

One property of the jump also has to be stated, because without it no mission could stop
anywhere: **a jump ends moving with whatever it jumped to.** The fleet cruises at a few per cent of
light speed (gravity-and-movement.md: "Getting a mothership to 0.05c"), while a body out there is
nearly at rest. A craft that kept the fleet's speed could never land on anything, and could never
come home.

### 3.2 The starting range, and what lies inside it

**X = 1 light-year around each mothership to start**, his number. That is 63,241 astronomical units
(AU, the Earth-Sun distance) in every direction (computed).

What a light-year sphere between the stars holds (computed from these sources, Appendix B):
- **Small icy and rocky bodies, uncountably many.** Objects like 1I/'Oumuamua (a few hundred metres
  across) are estimated at about 0.2 per cubic AU of interstellar space: Do, Tucker and Tonry,
  "Interstellar Interlopers: Number Density and Origins of 'Oumuamua-like Objects", *ApJ Letters*,
  2018 (arXiv:1801.02821, posted January 2018); a later review quotes about 0.1. A 1-light-year sphere is
  about 1.06e15 cubic AU, so it holds on the order of 1e14 such bodies. Water, carbon and metals
  for the fleet: the everyday mission.
- **A planet, rarely.** Free-floating planets may number about 21 for every star (Sumi and others,
  "Free-floating Planet Mass Function from MOA-II 9 yr Survey toward the Galactic Bulge", *AJ* 166,
  2023), though a 2024 model predicts about 2 (arXiv:2407.05992). At an assumed 0.1 stars per cubic
  parsec near the Sun, a 1-light-year sphere holds about a hundredth of a star's share, so roughly
  one chance in four (or in forty) of a rogue planet. A planet inside the link is an event, the kind
  a server remembers.

[closed-loop-test.md](closed-loop-test.md) says "There is no mine between the stars". That is true
without jumps, and it is the honest reason the fleet's loops are closed. With jumps, the mine is every
one of those bodies, and the closed loops become the fleet's insurance (his 2026-10-05 proposal:
ships built "to survive indefinitely" if the jump is ever lost).

### 3.3 The jump's reach, and the frontier

His sentence couples the two numbers: "The motherships broadcast X range from the ship meaning we
could only jump 1 light year out". This design keeps his coupling on day one and changes one thing,
saying why.

**Recommended: two numbers, the same at the start, upgraded separately.**
- The **link range** is how far around each mothership (and each beacon) a mind stays on two
  supports. 1 light-year to start.
- The **jump reach** is how far one jump can take a craft from where it starts. 1 light-year to
  start, and a jump starts from a mothership or a beacon.
- On day one they are equal, so everything a new player can reach is inside the link: no death can
  be final for anyone who has not chosen to go further. That is safe by default.
- When the crew's upgrades take the jump reach past the link range, **the band between the two is
  the frontier**: places a craft can reach but where minds are on their own. Beacons set down in the
  frontier pull the link out over it. Reach, then hold: the film's expansion loop at the scale of a
  mission.

**Why not his coupling exactly (jumps end only inside the link):** then nobody can ever be beyond the
link except when a beacon fails while people are past it, or in a story place. Flying the last
stretch past the edge under a craft's own drive is not a way out either: at 0.1 g from rest, 10 days
of thrust covers 2.45 AU, a 26,000th of a light-year (computed), and on the real-time server that
is ten real days. So the coupled rule makes recorders and Hardcore nearly meaningless, which his
"How would death out there be final?" suggests he does not want. Question 4 asks.

### 3.4 The fleet's upgrades: range, reach, speed and "better guns"

| Line | What it changes | Starts at (assumptions) | Tied to |
|---|---|---|---|
| Link range | the radius around each mothership | 1 light-year | the beacons' own radius is a separate line |
| Jump reach | how far one jump goes, and how long a drive needs between jumps | 1 light-year | the frontier opens when reach passes range |
| Cruise speed | how fast the motherships cross between the stars; also craft speed inside a system | a few per cent of light speed | point defense, below |
| Point defense | how much dust and debris the fleet can clear ahead of itself | enough for the starting speed | cruise speed |

**Speed and defense are one problem.** A grain's energy grows with the square of its speed. At 0.05c
a 1 mg grain carries 1.1e8 J, about 27 kg of TNT; a 1 g pebble carries about 27 tonnes. At 0.1c the
same grains carry about 108 kg and 108 tonnes (computed, Appendix B). So "fly faster" is only safe
with "better guns", and the fleet's top safe speed should be a function of its point-defense level,
in data. Two of his wishes become one honest piece of engineering.

**How an upgrade happens, all from systems that exist:**
- Anyone proposes one, and the crew votes on which comes next: signed governance proposals and
  votes are live (docs/FEATURES.md, "Native Governance page").
- The crew builds it from contributions: materials and work given to a fleet project, written as
  Contributed lines in the fleet ledger (data/ship/fleet_ledger.ron).
- Each upgrade is a data row (proposed: `data/fleet/upgrades.ron`, with id, line, level, effect,
  cost and what it needs first); a server's fleet holds its own levels in a small table; a solo
  game's fleet is the player's own.
- Progress shows on a fleet projects board in the Concourse and on a page in the app, so it is
  reachable without entering the world (the GUI-first rule; the web mirrors it).

### 3.5 What weapons are for, in a humanity at peace (question 5)

His list includes "better guns". The recommendation: **the fleet's weapons point at things that are
not people.**
- **Dust and debris at cruise speed.** The real reason a starship needs guns (3.4). Crews man the
  point-defense turrets through a debris storm: the turret minigame already designed
  ([docs/game/turret-defense-minigame.md](../game/turret-defense-minigame.md); a `defense_turret`
  marker sits in data/machines/home.ron) becomes the fleet's, not one home's.
- **Animals and hazards on strange worlds.** Combat exists for exactly this: predators hunt the
  player and bites land as damage (CombatSystem v0.760, AISystem v0.761; tests/engine_wiring_lint.rs),
  behind the Hostile wildlife setting.
- **Tools that cut and drill.** A mining laser and a cutting torch are the same machines.
- **The unknown.** The galaxy is silent, not proven safe. The main story's destination mystery,
  Agartha, is ringed by the wreckage of others' wars (the film's canon). Preparedness without an
  enemy is the most realistic reason of all.
- **Sport, by consent.** The ship already has an Arena and an Armory (districts in
  data/blueprints/ship_structure.ron). Duels and target shooting happen there, between people who
  both chose it.

**Never people against people.** No raiders and no player war: people settle conflict by vote and by
the Accord. The turret design's "raiders" should become debris and the unknown. Consensual sport in
the Arena is the only fighting between people.

### 3.6 Beacons, which players build

- **The name.** "Beacon", not "relay": in this project the relay is the server program, in the code
  and in the glossary (`[[relay]]` in data/i18n/en.json), and the terminology rule wants one word
  for one thing. "Beacon" is also the film's word ("light its beacon") and fits his "broadcast".
- **What it is.** A machine players build from a bill of materials, carried by a craft and set down
  inside the link or in the frontier. It holds the link around itself (assumption: 0.5 light-year to
  start, its own upgrade line) and is a starting point for jumps. It belongs to the fleet, with its
  builders credited on it and in the ledger: shared infrastructure that outlives its makers, which
  gravity-and-movement.md already called the project's thesis as a mechanic (of gates).
- **Upkeep and failure.** It needs power and service, and debris can hit it. A beacon that fails
  takes its part of the link with it, and everyone past it is suddenly beyond the link: the rescue
  mission writes itself. Service runs are a standing reason to fly.
- **The thread home** (question 3): if the fleet set down a beacon behind it every light-year since
  it left, the link reaches Earth, transports can bring new crew from Earth, designs arrive at once,
  and a trip home is a chain of jumps.
- **On a server:** a small table of beacons (id, position in light-years, radius, power, state),
  changed rarely and told to everyone in the world when it changes.

### 3.7 How range shows

- **The Maps page.** The Galaxy view already draws the real HYG catalogue (109,400 stars at their
  positions in light-years: docs/FEATURES.md, "Maps Galaxy View on the Real Catalog"). Added: the
  fleet's position; the link as a soft bubble around each mothership and beacon; each beacon's
  state; the jump reach as an outline; the frontier band between them; every destination marked
  inside the link or beyond it by how far.
- **The HUD.** A small link mark appears in the last tenth of the range and beyond it (strong, thin,
  none), and beyond the link it shows the recorder: who holds it, and in Hardcore the power left.
- **The departure board.** A trip that ends beyond the link says so before you board, with what a
  death there would cost in your mode.
- **The death screen.** Inside the link or not, and where you will wake, or where your recorder is.
- **Precision.** Positions in light-years are f64 (`DVec3`) end to end, the CLAUDE.md planet-scale
  rule. "Is this point inside the link" is a union of spheres over a handful of sources, tested
  about once a second.

---

## 4. Death beyond the link

**Reworked by [respawn-and-revival.md](respawn-and-revival.md) (2026-11-03), after his answer in
10.1:** the recorder below is replaced by one heavy field lifeline per team, taking every back slot;
that design's section 2.11 holds the table that replaces 4.2, and its section 3 holds Hardcore's
costs. This section stays as written until he accepts it.

### 4.1 The recorder

A recorder is a portable second support for the mind: a small hardened unit worn at the collar.
While you live it runs beside your brain as the link does. When you die beyond the link it keeps
you, dimmed. When it comes back inside the link it rejoins, a respawner grows your body, and you wake
remembering everything, the dark time included, because nothing stopped. (It takes the aviation
"black box" as a form, and only that.)

- **Where it lives.** Simplified and Realistic: crew issue. Everyone whose trip leaves the link is
  fitted with one at departure; it cannot come off out there, and it cannot fail. Hardcore: real gear
  in an equipment slot, carried by choice, which can be left behind, lost or destroyed, and which
  runs on a battery.
- **How it comes home.**
  - Simplified: by itself, in about a minute (assumption; data).
  - Realistic: the fleet's recovery run collects it from where your trip said you were going, on a
    timetable (assumption: within an hour of real time), or sooner if anyone carries it into the link.
  - Hardcore: the same run and the same carrying, but the recorder can fail. It is destroyed with
    the body by a cause that leaves nothing (a list in data: inside a star, a drive or reactor blast,
    burning up on entry, and so on); its power runs out first (assumption: 72 hours of real time); or
    it is somewhere the run cannot find, because you went off your trip's plan, and nobody who knows
    brings it.
- **While you sleep.** A dim view of what the recorder senses, faint and slow: the dark, the hum, the
  voices of whoever is near it on voice chat. You cannot act; you can sign off and the recorder
  carries on. In a solo game the wait can be slept through like a night (as leaving-the-ship.md lets
  a solo player wait out a trip); in a shared world it is real time.
- **What ends a character.** Only the mind's last support stopping, which only Hardcore allows.
- **Every number above is data**, and question 6 asks him to confirm them.

### 4.2 Every case

| | **Simplified** (the default) | **Realistic** | **Hardcore** (chosen per character at check-in) |
|---|---|---|---|
| **Inside the link** | You wake at once in your chosen respawner. Nothing is lost: the fleet brings what you carried back with you. A body line in the ledger. | You wake at once. Your backpack lies where you fell, in one pack, for 60 minutes of play; what you wore and had equipped is re-issued. A body line. | Exactly as Realistic. Inside the link Hardcore costs nothing more: the mind never stopped. |
| **Beyond the link, with a recorder** | You sleep about a minute while the recorder comes home by itself, then wake remembering everything. The pack lies where you fell (what a Realistic death costs today). A body line. | You sleep until the recorder is carried into the link: by anyone, sooner, or by the fleet's recovery run. The pack lies where you fell; what you wore is re-issued; a body line; the time asleep. | As Realistic, but the recorder can fail. Destroyed, out of power, or never found: the character ends (4.3). |
| **Beyond the link, without a recorder** | Cannot happen: everyone whose trip leaves the link is fitted with one, and it cannot come off out there. | Cannot happen, for the same reason. | The character ends at that death: the mind had one support. The departure board and the HUD say so before and while you are out there. Nothing stops you going. |
| **What a death can cost** | a little time, beyond the link | the pack, and the time asleep | everything the character was, if the last support stops |
| **What the person always keeps** | everything they know | everything they know | their account, name, look, friends and standing, even when the character ends |

This answers (d)'s open case: someone beyond the link with no recorder at all is final in Hardcore,
and in Simplified (and Realistic) cannot exist.

### 4.3 A final death without banning anyone

His questions in (c), one at a time:

- **"How would death out there be final?"** Because the fleet never wakes a copy (rule 3 of 2.1),
  and the link keeps nothing it could wake. When the last support stops there is nothing to restore.
- **"How would we enforce that?"** On a server with closed or hybrid custody
  ([characters-and-servers.md](characters-and-servers.md)), the server holds the Hardcore choice and
  the end: an ended character cannot join that world again as itself, and the person's next arrival
  is a new character (a generation number per person per world, so nothing about the person is
  deleted). On an open server and in a solo game the save is the player's own, so Hardcore there is
  the player's word, and the game should say that plainly rather than pretend.
- **"The player still wants to play on the server."** They do, at once. The next transport brings
  them in as a new crew member, and the intro plays again (skippable after the first time).
- **"Do we really want to ban people?"** No. A ban is for conduct, decided by game moderation
  (characters-and-servers.md, "Game moderation is separate from chat moderation"). Dying is play.
- **"What are our other options?"** Each was weighed:
  1. A ban: rejected; it punishes a person for playing.
  2. A cooldown before a new character: rejected; it punishes the person, not the character, and
     the next transport's wait is already a natural pause.
  3. A skill or experience penalty on respawn: rejected; it breaks "They maintain all of their
     knowledge".
  4. Waking from a backup: rejected; it is a copy, the orchestrator's corrected option 1.
  5. **The character ends and the person continues: chosen.**

**What an ended character leaves.**
- **Its home** passes to the person it named at check-in: an heir, written as a signed note naming
  another person's key (the certificate pattern the plot permits already use). One home per person
  per mothership still holds, so the heir may move in (moving house) or take the contents into their
  own storage. With no heir, the plot returns to the fleet and its contents go to the fleet's
  stores. Recommended: the heir cannot be the person's own next character, because the stake of
  Hardcore is the whole life (question 7).
- **Its memory.** If the person agreed at check-in (off by default: the safe-by-default rule), a
  memorial in the Concourse names the character, the days they lived aboard and what they built.
- **Its save** is set aside, never deleted, as an erased account's server save already is
  (crates/humanity-systems/src/persistence/live_save.rs, `set_aside_erased`).

**Leaving Hardcore.** Recommended: a person may take a character out of Hardcore at any time (lives
change), but a character never goes back in, and its record shows how long it was Hardcore
(question 7).

### 4.4 What changed from the orchestrator's answers, and why

- **Option 1 of (d) woke a copy.** (d) corrected it; this design uses the correction.
- **Realistic was left undefined.** Defined here: the recorder is crew issue and cannot fail, but
  nothing brings it home by itself; the fleet's recovery run or a friend does.
- **The no-recorder case.** Answered: final in Hardcore; impossible otherwise.
- **Joined to the link at boarding, not at check-in.** The orchestrator's note on (e) made check-in
  the moment you join the link. Then a new arrival who walked past the desk and died inside the link
  would have no second support, and by rule 3 would be dead for good: a new player's first death
  final by accident, and "general services" a contradiction. CLAUDE.md's 2026-10-09 rule is safe by
  default, so every passenger is linked when they board the transport, and check-in records where
  they wake. The desk is still where the link is explained, as the note wanted.
- **The link keeps nothing.** No backup exists anywhere, so there is no exploit to close and no
  temptation to "just restore it this once".

---

## 5. The intro, beat by beat

### 5.0 When it plays, and when it does not

- **A new character's first arrival on a world**: a server's world, or the person's own solo home.
  That is the moment the game makes a fresh save today (src/engine/save_slot.rs `fresh_save`; in
  Characters, "A new home" or a server card's Choose).
- **Again after a Hardcore character ends**: the next transport (4.3).
- **Never for a returning character.** They wake where their save left them, as today.
- **Characters (Play) stays** as the place to pick who and where
  ([play-characters.md](play-characters.md)); the ride begins after its Enter, for a new pairing.

### 5.1 The beats

| # | Beat | What the player sees and does | Underneath | About how long |
|---|---|---|---|---|
| 1 | Out of the jump | Black, then sound: the hum of a small craft coming out of its jump at the mothership's gateway (question 12), the pilot's voice naming the mothership ahead. | The ride's own small scene loads first (cabin, seats, window, stars); the world starts loading behind it. | a few seconds |
| 2 | The window seat | You sit at a window, or in the front passenger seat. The camera is yours: look around, never leave the seat. Others sit with you; cargo is strapped down behind a net. Out the window: 5.2. | The ship's meshes, the plots and homes, shader warm-up, and in a shared world the join, all behind the view. | 60 to 120 s the first time; it fits the trip and the loading |
| 3 | The approach | The mothership grows from a point of light into a city. The drums turn. The drive glows far aft. The transport matches the ship's push, lines up with Hangar Bay 1 and passes in. | If loading is not done, the transport holds outside the doors: "waiting for a docking slot", which is true, since it is waiting for your world. | about 30 s |
| 4 | Docking | Clamps, the hiss of air, the door opens. | The server's welcome stands each passenger at the transport's door, in seat order. | about 10 s |
| 5 | Stepping off | The hangar, as the cargo comes out: loaders and crew moving crates away, other craft in their cradles, people. **Full control from here: the first free minute starts as you step off.** A sign and a crew member point to Arrivals. | Cargo is drawn on your machine; the crew are the relay's ambient crew. | yours |
| 6 | The Arrivals desk | Walk up, press E: the receptionist (5.4). | The plot is claimed here, not at join. | 1 to 3 minutes, as long as you like |
| 7 | The way home | The receptionist gives the opening quest; its first step is "Go home". The HUD marks the way. The walk shows the ship's inside: the Concourse, the drum's curve, First Street. | The Travel objective with `OwnFrontDoor` exists already (docs/FEATURES.md, "The opening"). | a few minutes |
| 8 | Your door | Your home. The start's opening carries on (First Steps for a Homestead, Settling In for a Cabin) and ends, as now, at your front door looking out. | unchanged | |

### 5.2 The ride

- **The seat.** A window seat or the front passenger seat, his two pictures. In a shared world the
  server's seat number decides which; solo, the front seat.
- **The craft.** A small transport with a dozen seats or more (his "a doze or more") and a cargo hold.
  It is a design file built with the same construction tools as homes and the ship, which is the
  leverage the-mothership.md names ("A bad building experience for player or dev means we don't get
  as good as looking spaceships"). Its look is our own (Appendix A).
- **Out the window, by where the fleet is** (question 2):
  - *Between the stars:* the real stars as seen from the fleet's own position (the HYG catalogue in
    data/stars-map.bin, drawn by src/renderer/stars.rs; a few light-years out the near stars have
    shifted and the Sun is one bright star behind). No sun, so the mothership is lit by itself: the
    windows of homes and farms, the drum rims, the drive's glow. Other ships of the fleet far off. A
    beacon's light, where one is near.
  - *At Earth (today's code):* the ship in its synchronous orbit over Silverdale, Earth below. This
    view renders now.
- **The mothership from outside.** The hull wrap exists but is visual only and wraps the built part
  of the ship (crates/humanity-ship/src/hull.rs; docs/FEATURES.md "The Hull Wrap"). A true-scale
  exterior of a ship that is home to more than a billion people is new work (rung 6).
- **The people beside you.** Other players on the same trip sit in their seats with their own look;
  their idle movement is drawn on each machine and never sent; names show only as each person's
  nameplate choice allows. A solo game fills the seats with crew figures, the cosmetic tier
  the-mothership.md welcomes.
- **The pilot's lines** name the mothership you are arriving at (the server's name) and where the
  fleet is. Arriving somewhere is never silent, which answers the first-hour audit's first blocker
  (the first session that quietly joined the shared world).
- **Loading, hidden.** The ride is a timeline that ends at the trip's docking time. Its scene loads
  first; everything else loads behind it. If loading finishes early, the ride plays at its natural
  length; if it is late, the transport holds outside the bay until your world is ready. No loading
  bar is ever shown during the ride.

### 5.3 Stepping off

- **The hangar.** Hangar Bay 1 exists as a district (data/blueprints/ship_structure.ron: origin
  (200, 0, 0), 50 by 12 by 40 m, drawn with ship cradles by `structure::filler_parts`). New: a
  walkable bay with an Arrivals hall beside it, and a way from there to the Concourse and First
  Street. Cargo Freight is a district already (origin (-190, 0, 60)): where the crates go.
- **The cargo.** First, client-local animation: crates onto loaders, carried away (cosmetic, no
  network). Later, once shipments exist (ship-homes increment 9), the hold carries the arrivals'
  own things: each new home's starting storage arrives as a real shipment, so "goods always sit in
  exactly one place" (ship-homes section 0) is true from the first minute.
- **Seen from outside.** Players already in the hangar see the transport come in: a small note to
  players in view of the bay (the trip, the bay, the docking time, how many aboard) lets each
  machine draw the craft's approach itself.

### 5.4 Check-in: the Arrivals desk

The receptionist (his "admin guy like a receptionist") is a crew member at a desk. The relay's
ambient crew already work chores at sites, with nameplates (docs/FEATURES.md, "Crew Chore AI"); a
desk is one more site. The conversation is a panel on each player's own screen, so nobody queues
behind anybody, however many arrive at once.

What the desk does, in order. Every card has its default, so pressing on through it is quick:

1. **Welcome, and the link** (the lines in 2.2). Often this is the first time the person has pressed
   E in the world, so the desk says so, and shows the keys the first-hour audit found nobody is
   told: I for the inventory, E to use things, F1 for every key, and the help button.
2. **A bedroom of your own.** "Everyone aboard has a room of their own." This is the reading of his
   "Start the player off with a bedroom of their own" (question 10): every start includes a bedroom
   that is yours alone, given here. A Cabin is exactly one; a Homestead has one.
3. **Where you live**, his "play style (simple home, advanced, or whatever)". The existing "Choose
   where you live" cards (src/gui/pages/where_you_live.rs, the same widget Characters and Settings >
   Gameplay use), read from data/homes/starts.ron:
   - "simple home" is the **Cabin** (one private room; free crew meals in the mess hall; nothing at
     home can break or die);
   - the **Apartment** stays greyed until the first apartment block exists, as today;
   - "advanced" is the **Homestead**, the default ("most people start here");
   - the household where a start has one (Just you, or A family of three).
   - The server claims the plot here, through the message that already lets a player choose while
     holding none (`game_choose_home`, src/relay/handlers/home_plots.rs), and no longer at join.
   - If that kind of home is full, the waiting list of ship-homes 1.5, and general services meanwhile.
4. **How hard the world pushes back:** Simplified (the default), Realistic, or Hardcore for this
   character (the rules of 4.3: leave it any time, never go back; name an heir; a memorial only if
   you say yes). Settings > Gameplay > Death keeps Simplified and Realistic; Hardcore shows there as
   a fact with its "leave Hardcore" button. For children the desk suggests the Cabin and Simplified
   as a pair, as ship-homes question 18 recommends.
5. **Where you wake:** your home's respawner (the default) or general services; more places later
   (5.6).
6. **What others see of you.** The privacy choice that today opens mid-screen in the world, with the
   mouse still captured (first-hour audit, step 7; `privacy_tier_prompt_open` in
   src/engine/input.rs), belongs at this desk. It is a real choice and this is the natural moment.
7. **The way home:** the opening quest, its first step "Go home".

**GUI-first.** Every choice made at the desk also lives in the app outside the world (Settings >
Gameplay, Characters), so nobody has to walk to a desk to change it, and a later visit to the desk
changes the same settings. The receptionist's lines and the desk's cards are data, never code.

### 5.5 Never checked in: general services

- **Who:** anyone who walked past the desk, and anyone whose kind of home is full (today's guest).
- **Where:** a General Services hall beside the Medical Bay (a district already, origin (200, 0,
  93)) and near Arrivals: the ship's public respawner, a barracks of bunks, the medbay. The room
  type for shared bunks already exists as "quarters" in data/rooms.ron (ship-homes 1.2 notes it).
  The bunks are beds you can sleep in; the first-hour audit found that only a bed you built works
  today (`built_uses.rs`).
- **What they can do:** everything shared (meals in the mess hall, recreation, the market, missions
  inside the link). Not building, since there is no home; not Hardcore, which is chosen at the desk.
- **If they die:** they wake at the public respawner and a bunk is theirs for the night. Never
  final, because they were linked when they boarded (4.4).
- **Reminders:** once on stepping off ("Arrivals is this way") and once after a first death ("Check
  in at Arrivals for a room of your own"). Never more.
- **Today's guest becomes this.** The guest's sentence (src/engine/home_plot.rs `guest_arrival`: "You
  cannot build, and the ship's shared places are open to you.") and the guest's respawn at the
  Commons become general services.

### 5.6 Choosing where you wake

- **Respawners are places, in data.** A respawner pod is a machine in a home design: in the
  Homestead beside the bedroom (recommended: you wake at home, where you sleep); for the Cabin block,
  one shared pod by the washroom, since a 7.5 m2 cabin has no room for one; the public pod in general
  services; later one in a planet field's shelter (leaving-the-ship.md question 8).
- **What exists.** data/rooms.ron has the room type `respawner` ("Respawn Chamber", equipment
  `respawn_pod`), used by the older layouts (data/blueprints/homestead_layout.ron,
  fibonacci_homestead.ron) and looked for by src/engine/world_load.rs for the avatar's podium. The
  shipped Homestead and Cabin have none, and today's respawn stands you at your front door (the
  Entry, data/homes/shipped/homestead.ron `spawn`). The room type's purpose reads "Molecular
  reconstruction pod for player respawn after death"; "reconstruction" reads like a copy, so its
  words should follow the lore: a pod that grows a new body for a mind that never stopped.
- **Allowed places:** your own pod; general services, always; a friend's pod if they gave you a
  permit (the certificate pattern of plot permits); a mission base's pod while you are on that
  mission.
- **The choice:** a default in the save (`wake_at`), changed at any respawner, at the desk or in
  Settings; the death screen lists the allowed places with the default first.
- **In a shared world** the server checks the choice (a permit, a place that exists) and stands you
  there through today's respawn path (`respawn_through_relay`).

### 5.7 Skipping it after the first time

- The first ride a person ever takes plays whole. After that (a flag in the person's config, not per
  world) a hint offers to skip: holding a key jumps to docking as soon as the world is loaded, and if
  it is not loaded yet the approach simply plays on.
- A Hardcore end's new arrival is skippable like any other.
- The desk always appears, but a returning person sees "Same as last time?" (their last start, mode
  and wake place) as one press, and walking past it is allowed.
- Settings > Gameplay > the arrival: first time only (the default), always, or never. Dev mode and
  the rigs pin it off with a showcase verb (proposed `{"arrive":"skip"}`), permanent dev tooling.

### 5.8 How it fits the first ten minutes and the first-hour audit

- **The opening quests start at home today** (docs/FEATURES.md, "The opening: the first ten
  minutes"): First Steps is check your vitals, eat, plant, craft a tool, get iron ore, smelt it,
  build a storage chest, step out of your own front door; the Cabin's Settling In is your vitals, the
  mess hall, a meal, the recreation room, home. The intro adds one step in front, "Go home" (Travel
  to `OwnFrontDoor`, an objective kind that exists), handed over by the receptionist. Nothing else
  in them changes. The opening now begins and ends at your door: you reach it from outside, and you
  finish looking out from it.
- **The first-hour audit's findings** ([first-hour-audit-2026-10-04.md](first-hour-audit-2026-10-04.md)):
  - Blocker 1, the first session silently joined the shared world: fixed since by onboarding's
    pairing; the pilot's lines now also name where you are arriving.
  - Friction 1, no guidance in the world: the desk teaches E and shows I, F1 and the help button.
  - Becoming a guest said nothing: general services and the desk say it.
  - The privacy window mid-screen with the mouse captured: moves to the desk (5.4, item 6).
  - Death was free: Simplified stays free by his decision; the lore gives every death a meaning and
    the wake place is a real choice.
  - The spawn just inside the front door (and data/world/spawn.ron saying "bedroom", read by
    nothing): replaced by an arrival.
- **The onboarding tour's "world" step** (data/onboarding/tour.json, done when `entered_world`)
  ticks when the transport docks.

---

## 6. Multiplayer

### 6.1 A dozen arriving on one transport

- **A trip is computed, never stored.** Arrivals are a line in data, the same shape as the departures
  ship-homes designed (its section 5.10: lines shaped after GTFS, the next trip computed from a start
  time plus the headway). Proposed `data/ship/arrivals.ron`: the bays, the craft and its seats (12 to
  start), the headway (assumption: 2 minutes on a busy server), the ride's natural length, and how
  long a trip waits for late boarders.
- **A seat.** A joining new character is given the next trip whose docking time is at least a ride
  away, and a seat on it; if it is full, the next trip, or the same minute's trip at another bay.
- **Together.** Everyone on a trip shares its docking time. Someone who joins later in the boarding
  window starts their ride further along the same timeline, so all of them step off together.
  Friends who join as a party are seated together; otherwise seats go in join order.
- **Nothing streams during the ride.** The server holds each passenger as "aboard trip T, seat S",
  not as a position. At docking its welcome stands each at the transport's door in seat order, and
  the normal position stream begins under data/ship/shared_world.ron's in-view rules.
- **Each machine loads at its own pace.** A slow one holds outside the bay on its own screen while
  the others step off; everyone else sees that person step off a little later. The transport leaves
  when its last passenger is out.
- **Privacy.** The server keeps no record of who arrived with whom: a trip's seats live in memory and
  are gone at docking (the house pattern of ship-homes 5.10, "Privacy"). Co-passengers see names only
  as each person's nameplate choice allows.

### 6.2 At MMO scale

- More arrivals an hour means more trips and more bays, never a longer ride.
- The server's work for one arrival: a seat answer of a few hundred bytes, one docking note to the
  players in view of that bay, and the welcome that exists. Nothing per frame.
- Check-in is a panel on each player's own screen, so a thousand arrivals an hour never queue; each
  plot claim is one `game_choose_home` transaction, as now.
- The ride costs the server nothing: it is drawn entirely on each player's machine.

### 6.3 A private server of a handful

- **On demand.** With the headway at 0 (a Server Settings row), a trip is made when someone joins,
  and anyone else who joins in the next 30 seconds (assumption) shares it. A dozen friends signing in
  on a Friday evening arrive together.
- The admin sets the seats, the headway, the bays and whether Hardcore is offered, in Server
  Settings (the GUI-first rule).

### 6.4 Solo

- Your own transport, on demand, with crew figures in the other seats. Your own fleet, beacons and
  upgrades; the recovery run is the crew's. Hardcore alone is the player's word (4.3).

---

## 7. What exists and what is new

| Piece | Exists today (where) | New in this design |
|---|---|---|
| Death, the death screen, Respawn | since v0.745: death at 0 health with a cause, "YOU DIED", a Respawn button (src/gui/pages/hud.rs `draw_death_screen`); Respawn restores health, raises vitals to at least 60, clears effects (src/lib.rs, `pending_respawn`) | the lore's words; a list of where to wake; inside or beyond the link; the dim view |
| Where you wake | your own front door (`fps_spawn`, src/engine/home_plot.rs; the shipped designs' `spawn`); a guest at the Commons; through the relay in a shared world (`respawn_through_relay`) | respawner pods as places; `wake_at` in the save; general services |
| The respawner room type | data/rooms.ron `respawner`, used by older layouts and looked for by src/engine/world_load.rs | in the shipped Homestead and the Cabin block; purpose words that follow the lore |
| The death pack | crates/humanity-systems/src/death_pack.rs, src/engine/death_pack.rs, data/world/death.ron | unchanged; also the cost of a Simplified death beyond the link |
| The death mode | Settings > Gameplay > Death, Simplified or Realistic (`death_realistic`) | Hardcore, per character, chosen at the desk, leave-only |
| Starts and "Choose where you live" | data/homes/starts.ron; crates/humanity-ship/src/starts.rs; src/gui/pages/where_you_live.rs; Characters; the relay's `assign_home` (src/relay/handlers/game_state.rs) and `game_choose_home` (src/relay/handlers/home_plots.rs) | the same cards at the desk; the plot claimed at check-in instead of at join |
| One save per world; the look crosses | src/engine/save_slot.rs (`Look`, `fresh_save`); crates/humanity-systems/src/persistence/live_save.rs (`set_aside_erased`) | save fields: checked in, `wake_at`, Hardcore, heir, a generation per person per world; the person's "arrival seen" flag |
| Guests | src/engine/home_plot.rs `guest_arrival`; respawn at the Commons | general services |
| The opening quests | data/quests/getting_started.ron (`gs_first_steps`, `gs_first_steps_cabin`); destinations `OwnFrontDoor` and `NearestVolume` | a "Go home" step in front |
| The onboarding tour | data/onboarding/tour.json, src/gui/first_minutes.rs | its world step ticks at docking |
| The ship's districts | Hangar Bay 1, Medical Bay, Cargo Freight, the Concourse, Transit Hub, Armory, Arena, as labelled volumes drawn with filler parts (data/blueprints/ship_structure.ron) | a walkable bay, an Arrivals hall, General Services, the way to First Street |
| Ambient crew | relay-side crew working chores at sites, with nameplates (docs/FEATURES.md, "Crew Chore AI") | the receptionist's desk; crew in the hangar |
| Mission departures | designed (ship-homes-and-logistics.md sections 3 and 5.10), not built | arrivals in the same shape |
| The fleet ledger | data/ship/fleet_ledger.ron; src/relay/handlers/fleet_ledger.rs (meal, item, power) | kinds `body` and `reissue`; contributions to fleet projects |
| Governance | live: signed proposals and votes (docs/FEATURES.md, "Native Governance page") | votes on the fleet's next upgrade |
| The galaxy and the stars | the Maps Galaxy view on the HYG catalogue (data/stars-map.bin); src/renderer/stars.rs | the fleet's position, the link's bubbles, beacons, the frontier |
| Positions between the stars | `ContainerRef::Deep` designed (cosmos-architecture.md sections 4d and 10), with no call sites (gravity-and-movement.md) | the fleet between the stars, in light-years, f64 |
| Where the ship is | a synchronous orbit over Silverdale (data/stations/home.ron:33-45); First Street lit through glass | where the fleet is, per world; light aboard that does not need the Sun |
| The ship from outside | the hull wrap, visual only (crates/humanity-ship/src/hull.rs) | a true-scale exterior for the ride |
| Combat | CombatSystem (v0.760) and AISystem (v0.761): predators, bites; the Hostile wildlife setting | point defense; the turret minigame retargeted at debris |
| The jump | none: no FTL decided 2026-09-15, reopened 2026-10-05, awaiting his answer | the jump, if adopted (question 1) |
| Body mods | none | the rule for when they exist |
| The link, beacons, recorders, the recovery run | none | all new |
| The older intro, "The Dream" | designed, never built (docs/game/intro_sequence.md) | replaced by this, if he approves (question 13) |
| Rig verbs | `{"die":...}` and `{"respawn":"1"}` (src/engine/ipc.rs) | `{"arrive":"skip"}`; verbs to stand a rig beyond the link and to switch a beacon off |

---

## 8. Budgets

Per CLAUDE.md, each effect states its per-frame budget and its network footprint before it is
built. The target is the operator's machine at his display's 2560x1440 and 120 fps, where a frame is
8.3 ms. These are budgets to measure against, not measurements.

| Effect | Runs where | Per-frame budget | Network |
|---|---|---|---|
| The ride: cabin, seats, people, window, stars, the mothership far off, the drive's glow | each player's machine | GPU 4 ms and main-thread CPU 3 ms, leaving the loading threads the rest and the frame under 8.3 ms | none during the ride; one seat answer, a few hundred bytes, per arrival |
| Co-passengers' idle movement | each machine | inside the ride's budget | none |
| The approach and docking | each machine | inside the ride's budget | the welcome, as today |
| A transport docking, seen from the hangar | each machine | 0.3 ms | one small note per trip, to players in view of the bay |
| Cargo coming out (crates, loaders) | each machine, instanced | CPU 0.3 ms, GPU 0.5 ms | none |
| Crew in the hangar and at the desk | the relay's ambient crew | the crowd's own budget ([crowd-simulation.md](crowd-simulation.md)) | the existing crew updates, to players in view |
| The desk's panel | each machine | 0.5 ms while open | the plot claim (an existing message); the mode and wake choices, a few bytes |
| The link mark and the inside test | each machine | 0.05 ms; the sphere test once a second | beacon changes only, rare |
| The dim view while asleep | each machine, a post-process | 0.3 ms | none (voice as today) |
| Waking in a pod | each machine | 0.3 ms | the existing respawn path |
| The map's bubbles, beacons and frontier | each machine, on the Maps page | 1 ms on that page | the beacon table once on joining, then changes |
| Recovery runs and recorder clocks | the server | nothing per frame: a sweep once a second, like ship-homes 5.10's deliveries | a row per waiting recorder; one message when it comes home or the character ends |
| The fleet's upgrades | the server | nothing per frame | a row per line, told on change |

Everything cosmetic (the ride, the cargo, the people's idle movement, the dim view) is drawn on each
machine and never networked; only gameplay state (a seat, a plot, a wake place, a recorder's fate)
is synced, at a low rate. Rung 4's gate measures the ride's frame time while the world loads, on his
PC, with his share of it kept (no raised priorities).

---

## 9. The rungs to build it

Each rung is playable and is a rung of the real thing, not a throwaway (the realistic-first rule).
Rungs 1 to 7 need no FTL decision.

0. **His answers** to section 10. Gate: his review of this document.
1. **The words.** The Library pages "The Fleet", "Coming back as yourself" and "What is made up in
   this game"; glossary terms (the link or his word for it, beacon, recorder, respawner, general
   services, frontier); the respawner room type's purpose words. Gate: `node
   scripts/check-doc-links.js` at 0, the Library freshness check, the glossary and i18n tests, the
   terminology lint; he reads the pages.
2. **Respawners as places, and general services.** Pods in the shipped Homestead and the Cabin block;
   General Services (a public pod, barracks bunks you can sleep in, the medbay) in
   data/blueprints/ship_structure.ron by the Medical Bay; `wake_at` in the save; the death screen's
   list; guests wake in general services; the `body` ledger line. Gate: lib tests (a respawn lands on
   the chosen pod's floor on every storey; a guest wakes in general services; a bunk is a bed);
   `verify-copresence --plots` still passes in every order (Respawn goes through the relay); a rig
   dies with `{"die":...}`, respawns, and is judged where it stands.
3. **Arrivals and the desk.** A walkable Hangar Bay 1 with an Arrivals hall and a way to First
   Street; the desk and its panel (welcome and keys, where you live, the death mode, where you wake,
   privacy, the opening); the plot claimed at check-in; "Go home" in front of both openings; a new
   arrival who walks past the desk lives in general services. Gate: relay tests (no plot until check
   in; check-in claims one; a full kind waits); the opening tests (src/engine/crossing_tests/quests_opening.rs)
   walk both openings with the new step; the shared widget's tests; a copresence run where two
   arrivals check in, in both orders; a snapshot of the desk at 2560x1440.
4. **The ride.** The transport's design; the ride's scene and timeline; loading behind it, with the
   hold; the arrivals line and seats on the relay; docking into the hangar; the skip; the cargo, drawn
   locally. Gate: `just verify-runtime` boots and enters the world THROUGH the ride; a frame-cost run
   on the ride while loading, against section 8; a rig with scripted second players in which a dozen
   on one trip step off in seat order; the skip path; the solo path; the pilot names the world.
5. **The link, shown.** Link sources and radii in data (proposed `data/world/link.ron`), a beacon
   table on the relay (empty at first), the inside test in f64, the HUD mark, the Maps page's
   bubbles, the death screen's line. Gate: lib tests of the union of spheres in light-years, with
   points just inside and just outside the edge, and one that fails on f32 positions; a snapshot of
   the Maps page.
6. **The mothership from outside, at true scale.** A data-driven exterior of the drums, the spine
   and the drive at the size the lore states, with its levels of detail, for the ride and every
   outside view. Gate: the fidelity-expert's reference technique before the rung is fenced; visual
   sweep vantages from the transport and from First Street looking up; `just verify-runtime`; frame
   costs.
7. **Where the fleet is, per world.** "At Earth" and "Between the stars" as rows; light aboard that
   does not need the Sun; the sky drawn from the fleet's real position. The default flips to between
   the stars when this passes, if he chose it (question 2). Gate: both rows boot and enter the world;
   the planet trips of leaving-the-ship.md are offered only to a world at a planet; vantage
   screenshots of First Street and a farm under each row.
8. **Recorders and death beyond the link.** Needs a way past the link: the jump (question 1) and
   missions from the hangar. The recorder by mode; the dim view; the recovery run; Hardcore at the
   desk; ended characters held by the server, the heir, the memorial. Gate: a lib test for every cell
   of the table in 4.2 (nine); relay tests (an ended character cannot join as itself; the next arrival
   is new; the heir receives the home); rig verbs to stand beyond the link and to switch a beacon off,
   and a rig per mode.
9. **The fleet's upgrades and beacons.** The four lines as data; votes; contributions; the projects
   board and page in both apps; beacons with upkeep and failure; the turret minigame on point
   defense. Gate: relay tests (a vote picks the next upgrade; contributions complete it; a failed
   beacon puts the players past it beyond the link); snapshots of the projects page on both apps.

---

## 10. Questions only the operator can answer

Each has a recommendation.

1. **The jump.** Adopt your 2026-10-05 proposal, scoped like this: small craft jump, the motherships
   never do and cruise between the stars under thrust (which keeps the spine's gravity of
   2026-09-15), and the fleet can live without the jump forever. Recommend yes, with the jump named
   as made up (2.5). Without it the link still works (it is made up either way), but nothing can be
   reached beyond a few days' flight, and sections 3.3, 4 and rungs 8 and 9 wait.
2. **Where is the fleet now?** Recommend: between the stars, as the story's present and your MMO
   server's, once rung 7 passes; "At Earth" stays a row that a world can choose, which is what the
   code is today and what the Silverdale trips of leaving-the-ship.md need. This also answers
   leaving-the-ship.md's question 1 ("When is now?").
3. **Does the link reach Earth?** If the fleet set down a beacon behind it every light-year since it
   left, Earth is inside the link: new crew can come from Earth, designs arrive at once, and a trip
   home is a chain of jumps. Recommend yes; it is the strongest tie between the fleet and Earth's
   homesteads.
4. **Can a jump end beyond the link?** Recommend yes: the jump's reach and the link's range start
   equal (your 1 light-year, so nothing a new player can reach is beyond the link) and are upgraded
   separately, so a frontier can open and beacons can close it (3.3). The other way, jumps that end
   only inside the link, leaves almost no way for a death to be final.
5. **What are weapons for?** Recommend: things that are not people (dust and debris at cruise
   speed, animals and hazards on worlds, cutting and drilling, the unknown) and sport by consent in
   the Arena. No human enemies and no player war; the turret design's "raiders" become debris and the
   unknown (3.5).
6. **The recorder's numbers.** Recommend, all real time and all data: Simplified brings it home in
   about a minute; Realistic's recovery run comes within an hour; Hardcore's battery lasts 72 hours;
   and a short list of causes that destroy a Hardcore recorder with its body.
7. **Hardcore's rules.** Recommend: a character may leave Hardcore at any time and never go back;
   the heir cannot be the person's own next character; the memorial only with a yes at check-in, off
   by default.
8. **A body in the ledger.** Recommend: each new body is a Used line at a placeholder price of 0
   until body materials are goods, and re-issued clothes and gear a line beside it; nobody is ever
   refused a body, in the red or not.
9. **How long does waking take inside the link?** Recommend at once, in every mode. A death's cost is
   what it loses, never a wait for its own sake.
10. **"Start the player off with a bedroom of their own."** Read here as: every start includes a
    bedroom that is yours alone, given at the desk (the Cabin is one; the Homestead has one).
    Recommend that reading. The other reading, a bedroom aboard the transport, does not fit a small
    craft with a window seat.
11. **What do people call the link?** "Link" already means a web address on the app's screens ("One
    link a message"), and the terminology rule wants one word for one thing. Recommend "the
    lifeline" as the word people read, with "link" kept only in this design. If you prefer "the
    link", the glossary card should say it is not a web link.
12. **The gateway.** The journal's summary of your 2026-10-04 answers records one gateway per
    mothership, like a stargate, not one per player, which at first only lets players arrive.
    Recommend: the gateway is the point outside each mothership where transports come out of their
    jump; you arrive through it on the transport, not by walking through a door.
13. **The older intro.** docs/game/intro_sequence.md ("The Dream": a meteor, a dream of Earth
    destroyed, waking aboard) was never built. Recommend retiring it as the intro, keeping its
    subtext (Earth is fine; humanity left by choice), which this setting carries.

---

## Appendix A. The touchstones: what is borrowed as a form, and that nothing else is

His three touchstones are for a FEELING. This design takes the general forms below and nothing else:
no names, characters, places, ships, art, music, words or story beats. The craft, the ship, the
people and the lore are original.

| Touchstone | The form taken | Not taken |
|---|---|---|
| Half-Life (1998) | an opening ride seen in first person, the player in their own eyes and free to look around, the world going on outside, the ride covering the loading, no cutscene | its tram, its facility, its people, its words, its art, its music, its story |
| Star Trek | a small passenger craft with a front seat and a window; a humanity at peace that explores | every name (the craft is not called a runabout), ship design, uniform, insignia, organisation, species, device and story |
| Battlestar Galactica | a people who come back after death, as a fact of their lives | the Cylons, their ships, their "download", every name, character, design and story. Our mechanism is the opposite of theirs in the one point that matters: it never carries a mind across to a new body, because the mind never stops where it is |
| Stargate (the journal's summary of his 2026-10-04 answers: a gateway "like a stargate") | one fixed arrival point per mothership | the ring, its look, every name |
| The aviation flight recorder | a hardened unit that survives what its carrier does not, as a form; the name "recorder" is a plain word | nothing else |

Parfit, Moravec, Plutarch and the hemispherectomy research are cited as real thought and real
science, not as borrowed fiction. The beacon and the expansion loop are the project's own, from its
film. When this is built, references for the look come from real engineering (the fidelity-expert's
brief), never from another work's assets.

## Appendix B. Sources and computations

**Constants:** c = 299,792,458 m/s; 1 light-year = 9.4607e15 m; 1 AU = 1.495978707e11 m; 1 parsec
= 3.0857e16 m = 3.2616 light-years; 1 tonne of TNT = 4.184e9 J; g = 9.80665 m/s2.

- **A light-year in AU:** 9.4607e15 / 1.4960e11 = 63,241 AU.
- **A 1-light-year sphere:** (4/3) pi (63,241)^3 = 1.059e15 cubic AU.
- **Small bodies in it:** at 0.1 to 0.2 per cubic AU (Do, Tucker and Tonry, 2018; the later review's
  0.1), 1.06e14 to 2.12e14.
- **Rogue planets in it:** an assumed 0.1 stars per cubic parsec is 0.1 / 3.2616^3 = 2.88e-3 stars
  per cubic light-year; the sphere's 4.19 cubic light-years hold 0.0121 stars' worth; at 21 planets
  per star (Sumi and others, 2023) that is 0.25, at 2 per star (arXiv:2407.05992) 0.024.
- **A grain at cruise speed:** kinetic energy (gamma - 1) m c^2. At 0.05c (gamma = 1.00125): 1 mg,
  1.13e8 J = 27 kg TNT; 1 g, 1.13e11 J = 26.9 t. At 0.1c (gamma = 1.00504): 1 mg, 4.53e8 J = 108 kg;
  1 g, 4.53e11 J = 108 t.
- **A light-speed link:** in 10 ms light covers 2,998 km; the Moon at 384,400 km is 1.28
  light-seconds away.
- **Flying past the edge:** from rest at 0.1 g for 10 days, s = (1/2) a t^2 = 0.5 x 0.981 x
  (864,000 s)^2 = 3.66e11 m = 2.45 AU, which is 1/25,847 of a light-year.

**Sources** (each with its own date): Parfit, *Reasons and Persons*, Oxford, 1984; Moravec, *Mind
Children*, Harvard University Press, 1988; Plutarch, *Life of Theseus*; Price and others, *PNAS*
107(32):14508-13, 2010-08-10; Spalding and others, *Cell* 122(1), 2005-07-15 (PubMed 16009139);
Kliemann and others, *Cell Reports* 29:2398-2407, 2019-11-19 (doi:10.1016/j.celrep.2019.10.067);
Do, Tucker and Tonry, arXiv:1801.02821, 2018-01; Sumi and others, *AJ* 166, 2023
(doi:10.3847/1538-3881/ace688); arXiv:2407.05992, 2024-07. Read 2026-11-02 through search summaries
of the papers and their abstracts, not the full texts; a build that quotes a number from them should
check the paper itself.

## Appendix C. Stale or conflicting items found while writing this

1. **CLAUDE.md's "Real/Sim toggle" section** describes a toggle that
   [two-realities.md](two-realities.md) says was removed on 2026-07-16 ("do NOT bring it back"); the
   two realities are separated by navigation. engagement-modes.md, quest-chains.md and
   app-bottom-strip.md also still speak of a toggle. For the handbook's keeper and a doc-truth pass.
2. **docs/game/intro_sequence.md** ("The Dream") is replaced by this design if he approves (question
   13); it now carries a pointer here.
3. **docs/game/humanity_one.md's scale** (a 500 km ship for 10 billion) and
   [habitat-generation.md](habitat-generation.md)'s first drum (250 m by 2 km, about 780 acres)
   describe very different ships. Rung 6 must pick the exterior's real size.
4. **leaving-the-ship.md question 1** ("When is now?") is the same question as question 2 here.
5. **closed-loop-test.md's "There is no mine between the stars"** stays true without the jump and
   stops being true with it (3.2).
6. **docs/game/turret-defense-minigame.md's "raiders"** are human enemies, which a humanity at peace
   does not have (3.5).
7. **data/rooms.ron's respawner purpose**, "Molecular reconstruction pod", reads as a copy (5.6).
8. **game-modes.md section E** says "Health hits 0 and nothing happens today"; death has been real
   since v0.745 and has costs by mode since 2026-10-04.
9. **docs/FEATURES.md, "Where you live"**, cites `src/persistence/live_save.rs`; since the crate
   split the file is crates/humanity-systems/src/persistence/live_save.rs.
10. **data/world/spawn.ron** is still read by nothing (embedded only,
    crates/humanity-core/src/embedded_data.rs), as the first-hour audit found.
11. **"Relay" for the range extenders.** The orchestrator's (b) and (d) said "the fleet's relays";
    this design says beacons, because the relay is the server program (3.6).
12. **gravity-and-movement.md's "Decision: torch-ship brachistochrone, no FTL"** still stands while
    his FTL proposal waits. The jump of question 1 is for small craft only, which keeps that
    section's reasoning about gravity whole: the motherships never coast.

## 10.1 His answers (2026-11-03)

Quoted exactly. Questions not listed here are still open, and their recommendations stand until he
answers.

- **Questions 1, 5 and 11** (the jump for small craft only; weapons for things that are not
  people, and sport by consent; "the lifeline" as the word people read): "Agreed".
- **Bringing someone back, beyond the lifeline.** "And any way of bringing dead people back to life
  is someone bringing the heavy portable gear piece that makes it possible but weighs a ton. Like
  maybe it takes all back slots while on mission. Like a backpack." So what keeps a mind going out
  there is not a small recorder every crew member wears: it is a heavy piece of gear someone on the
  mission chooses to carry, taking every back slot. Section 4 is reworked in the respawn design
  (below).
- **Hardcore.** "I like the idea. It's brutal. We'll have to keep playing with it. Maybe hardcore is
  losing all gear and experience gained during the mission. Could also make death make the highest
  leveled skill lose the previous full level's worth of experience. Just any skill, whichever is
  highest. We'll have to put together a doc specific for infinitely respawning lore mechanics or
  maybe revives use magic, since I kinda want to include magic."

The respawn and revival mechanics get their own design document, as he asks, collecting the
options (the heavy carried gear, Hardcore's costs, the highest skill's lost level, and magic) for
him to play with.

- **Designed 2026-11-03: [respawn-and-revival.md](respawn-and-revival.md)** (the field lifeline,
  Hardcore's costs side by side, magic's options, everything respawning costs and gives; twelve
  questions for him in its section 9).
