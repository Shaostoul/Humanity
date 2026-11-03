# Respawn and revival: the field lifeline, Hardcore's costs, and magic

**Status: a design, written 2026-11-03 against main at f095b9b14. Nothing in it is built.** The
operator asked for it: "We'll have to put together a doc specific for infinitely respawning lore
mechanics". It collects the options for him to play with. Every recommendation is the author's (an
agent's) unless it is marked as his own words. Section 9 holds the questions only he can answer,
each with a recommendation.

It builds on [game-setting-and-intro.md](game-setting-and-intro.md) (the setting, the lifeline, one
mind on two supports) and replaces that design's recorder (its section 4.1) with the heavy carried
piece he described. It keeps [death-and-your-pack.md](death-and-your-pack.md) (Simplified and
Realistic, built) as the base every option here adds to.

**How claims are marked** (the convention of game-setting-and-intro.md):
- Repo facts cite a file, and a line or a name where it helps, read at f095b9b14. Lines drift.
- World facts cite a source and the source's own date.
- "Computed" means arithmetic on cited figures; Appendix B shows the working.
- "Assumption" means a number of ours that play should tune. Every one of them is data when built.
- Draft words (what a person would read) are marked as drafts. Words people read come from
  `data/i18n/en.json` by key when built; this design writes no UI.

---

## 0. In short

- **The field lifeline** (section 2). His heavy piece is a backpack-shaped machine that carries the
  lifeline with a team beyond the fleet's reach: the minds of the crew near it run on their brains
  and on it, as they do on the lifeline at home. It weighs 40 kg (assumption), takes every back
  slot, so its carrier wears no backpack, and holds the minds within 200 m of it (assumption). It
  runs minds; it stores nothing, so it never copies anyone. A death near it is sleep until it
  reaches a respawner; a death away from it leaves a short window for someone to bring it to the
  body. That is his sentence made into rules: "any way of bringing dead people back to life is
  someone bringing the heavy portable gear piece that makes it possible".
- **One lore device explains every mode** (section 1.3): modes differ by what the fleet issues you,
  never by different physics. Simplified and Realistic go out on crew issue; Hardcore goes out on
  its own kit.
- **Hardcore's costs, side by side** (section 3): the character ends; everything you had with you and
  the experience gained on the mission; the highest skill's last full level; and combinations.
  **Recommended:** a death that is caught beyond the lifeline costs everything you had with you and
  the experience gained since you last slept, and the character ends only when no support is left.
  The highest skill's lost level stays a dial, off, because the lore can only justify it for skills
  that live partly in the body. His to keep playing with.
- **Magic** (section 4): three honest options. **Recommended:** in the fleet's own world revival stays
  technology, and "magic" is the crew's word for what nobody aboard understands; spells are a clearly
  fictional layer that a world turns on, marked on screen wherever they appear, and never in
  anything the game teaches as real. Today, unknown to most, 94 fantasy abilities are already in the
  game and the simplest are castable by everyone in every world (section 4.1).
- **One table of everything respawning costs and gives** (section 5), checked against the three
  rules (never a gap, never two, never a copy).
- **The no-fail path stays whole** (section 6): Simplified loses nothing, anywhere.
- **Eight rungs, smallest first** (section 8); **twelve questions for him** (section 9).

## 0.1 The operator's words (quoted exactly)

**(a) On bringing someone back, 2026-11-03:**

> And any way of bringing dead people back to life is someone bringing the heavy portable gear piece that makes it possible but weighs a ton. Like maybe it takes all back slots while on mission. Like a backpack.

**(b) On Hardcore, the same day:**

> I like the idea. It's brutal. We'll have to keep playing with it. Maybe hardcore is losing all gear and experience gained during the mission. Could also make death make the highest leveled skill lose the previous full level's worth of experience. Just any skill, whichever is highest. We'll have to put together a doc specific for infinitely respawning lore mechanics or maybe revives use magic, since I kinda want to include magic.

**(c) Agreed, the same day** (game-setting-and-intro.md section 10.1): the jump is for small craft
only; weapons are for things that are not people, plus sport by consent; "the lifeline" is the word
people read for the link. This design uses "the lifeline" throughout.

**Earlier words this design must keep:**

- **2026-11-02**, the setting's first ask (game-setting-and-intro.md 0.1 (a)): "They maintain all of
  their knowledge but, lost everything physical, including any body mods/changes."
- **2026-10-04**, death by mode (death-and-your-pack.md): "Today the death screen says 'Nothing was
  lost.' I'd keep that as the Simplified mode. In Realistic mode, your carried items would stay where
  you fell for a while, to go back for."
- **2026-10-04**, carrying weight (crates/humanity-systems/src/encumbrance.rs, header): "slower
  walking in realistic mode and no jumping in nonzero G based on weight/mass. We can obviously carry
  heavier in low-g to zero-g but, mass still applies."

---

## 1. What this design keeps

### 1.1 The three rules

From game-setting-and-intro.md section 2.1, adopted there and unchanged here. A mind runs on the
brain and on the lifeline at once, every moment; it is never a backup.

1. **Never a gap.** The first-person feed never ends. At death it dims and brightens again on waking.
2. **Never two.** A body is grown only for a mind whose body has died, and only one body carries a
   mind at a time.
3. **Never a copy.** Nothing ever starts a mind from a stored state. A mind with no support left has
   died.

Every option in this document is held against these three. Where one would break a rule, section 5
says so and what it would cost the setting.

### 1.2 Knowledge is the mind

His 2026-11-02 words fix one more rule: knowledge is kept. Today's code keeps skills, quests and
recipes through every death (death-and-your-pack.md, "What is kept"; skills live in the save as
`WorldSave::skills`, src/save_load.rs). So any Hardcore cost that touches experience (section 3)
must be about something that is not yet knowledge, or about the body, and must say which.

### 1.3 One lore device for every mode: crew issue, or your own kit

Modes are "how hard the world pushes back" (the Arrivals desk's card, game-setting-and-intro.md
5.4). If each mode had its own laws of nature the lore would come apart. So this design uses one
device everywhere: **the modes differ by what the fleet issues you, never by physics.**

- **Simplified:** the fleet's crew issue covers everything. Its craft carry the field lifeline for
  you, its reach covers the whole mission, and the fleet brings what you carried back with you.
- **Realistic:** crew issue covers your life and your learning, never your time or your pack. The
  field lifeline cannot fail, but someone has to carry it; the crew suit keeps a body that died out
  of reach cold for hours, so a rescue always has time.
- **Hardcore:** you go out on your own kit. What it covers is what you brought, and kit can fail.

The player meets this sentence at the desk, as a draft: "Simplified and Realistic go out on crew
issue: the fleet covers for you. Hardcore goes out on your own kit."

---

## 2. The field lifeline

### 2.1 What it is

**A piece of the lifeline you carry.** Inside the fleet's reach (1 light-year around each mothership,
0.5 around each beacon: game-setting-and-intro.md 3.2 and 3.6) every crew member's mind runs on
their brain and on the lifeline. Past that edge the lifeline lets go, and a mind has only its
brain. The field lifeline is the machine that carries the lifeline out there: the minds of the
crew joined to it run on their brains and on it, the same way they do at home.

**Why it is heavy.** It runs a whole team's minds live, all at once, every moment, and has to keep
doing it through radiation, vacuum, heat, cold and impacts, on its own power, for days. That is a
hardened mind-support core, its shielding and its battery, on a carrying frame. Draft words for its
card: "The field lifeline carries the lifeline with your team. Stay near it, and a death out here is
only sleep."

**The name.** "Field lifeline", a draft (question 1). It says what it does in the player's word for
the link. Two names were set aside: "lifeline pack", because "your pack" already means what a
Realistic death leaves where you fell (death-and-your-pack.md), and the terminology rule wants one
word for one thing; and "recorder", the setting design's word, because a thing that records reads as
a copy, the one thing rule 3 forbids.

### 2.2 What it weighs and takes

**Every back slot, as he said.** Today there is one back slot (data/inventory/equipment_slots.json:
head, chest, legs, feet, hands, back), and the backpack is what goes in it (data/equipment.csv:
`backpack_small_0` adds 10 kg of comfortable carry, `backpack_large_0` 25 kg). So whoever straps on
the field lifeline takes their backpack off. If the back ever grows more slots (a belt pack, a
quiver), the field lifeline takes all of them: its equipment row declares "all back slots", not one
slot by name.

**40 kg (assumption).** His "weighs a ton" is read as a figure of speech: a real tonne is twenty times
what a body carries comfortably at Earth's weight, so nobody could move it. 40 kg sits between the
two loads armies plan foot marches around: an approach march load of 72 lb (32.7 kg) and the
emergency loads of up to 120 lb (54.4 kg) carried when nothing else can (US Army FM 21-18, *Foot
Marches*, 1990-06-01, superseded by ATP 3-21.18 on 2017-04-17; read through secondary summaries).
Heavy enough that one person is the carrier and knows it; light enough to walk with. Question 2.

**What that does under today's carrying rules** (crates/humanity-systems/src/encumbrance.rs, all
computed in Appendix B):
- A body carries 50 kg comfortably at 1 g (`Inventory::weight_capacity`), plus what worn gear adds.
  With the field lifeline on, there is no backpack, so no bonus: the carrier has 10 kg to spare for
  everything else before the load is over the limit.
- In Realistic carrying, a load past the limit slows walking by how far over it is, and stops jumps
  under gravity: a carrier who also picks up 20 kg walks at 80% speed (computed).
- The limit follows gravity: 132 kg on Mars, 303 kg on the Moon, no limit when weightless. So the
  field lifeline is a burden on a planet and easy on an asteroid, which is real.
- Mass still applies (his words): a 40 kg load cuts a jump's launch speed to 80% wherever you are
  (`mass_jump_scale`, computed).
- In Forgiving carrying (the default), it warns and changes nothing about movement. The carrying
  mode and the death mode are separate settings, so a player can have Realistic deaths without the
  slow walk.

**Two things the code must learn first** (rung 3):
- **Worn gear's own mass counts toward the load.** Today it does not: the inventory tick sums only
  what is in the backpack's slots (crates/humanity-systems/src/inventory/mod.rs, the weight tally in
  `InventorySystem::tick`). A 40 kg machine on your back that weighs nothing would break his rule
  that mass still applies. Counting worn mass is right for every item (a large backpack is 1.2 kg, a
  heavy jacket 1.5 kg, data/items.csv), so it is one honest change, not a special case.
- **The carried volume follows the back slot.** Today the backpack's 65 L is the inventory itself,
  worn backpack or not (`default_volume_capacity`). With the field lifeline on, the carrier holds
  only what its frame's pouches hold (assumption: 10 L).

**Taking it on means taking the backpack off.** At the craft, the backpack goes into the craft's
hold. In the field, a teammate takes it, or it is set down where you stand as a pack to come back
for. What do you leave behind to carry your friends? That choice is the role.

### 2.3 Who carries it: a role

- **One per team, as a rule of thumb.** A mission's departure (ship-homes-and-logistics.md section 3,
  "A departure is a record: transport, bay, time, seats and mission") gains a line: "Field lifeline:
  carried by ..." Anyone on the roster may take it; the departure board says who has.
- **It can change hands.** Handing it over takes a few seconds (assumption: 10 s to unstrap and
  strap on), so a tired carrier passes it on, and a carrier who must climb hands it up first.
- **It can be set down.** Set down at a camp it keeps running where it stands, and the team works
  within reach of it. A camp is a real choice: lighter backs, but a fixed place everyone has to stay
  near, and a machine that could be left behind.
- **Two can go.** A big team (the co-op target is a dozen, game-modes.md F) may take two; a mind
  runs on whichever is in reach, and on both when both are. That is still one mind on several
  supports, never two of anyone.
- **Credit.** The mission's record names who carried it. The fleet remembers who kept the team.

### 2.4 Its reach, and how many minds

- **Reach: 200 m around the unit (assumption),** wherever it is: on a back, set down, or aboard the
  craft. That is 12.6 hectares (computed), a dig site or a valley floor, small enough that a team
  has to move together and large enough to work in.
- **Minds: 12 (assumption),** a transport's seats (game-setting-and-intro.md 6.1).
- **Who is joined.** Everyone on the mission's roster is joined at departure. While alive and within
  reach, a joined person is on two supports; out of reach, on one. Coming back into reach rejoins
  them by itself. The carrier can add anyone else with one press (a rescuer, a friend met out there).
- **Simplified:** the reach covers everyone on the mission, wherever they are (1.3).
- **Upgrades.** Reach and the number of minds are natural lines for the fleet's upgrades
  (game-setting-and-intro.md 3.4), voted and built like the others.

### 2.5 A death within reach: asleep on the field lifeline

The body dies; the mind loses one support and keeps running on the other. The person is **asleep on
the field lifeline**: the dim view of game-setting-and-intro.md 4.1 (the dark, the hum, the voices of
whoever is near it on voice chat), unable to act, free to sign off.

**Waking needs a respawner within the field lifeline's reach.** Two kinds:
- **Inside the lifeline.** When the field lifeline comes back inside the fleet's reach, every mind
  on it rejoins the lifeline, and each wakes at their chosen place (game-setting-and-intro.md 5.6).
- **The craft's pod.** A mission craft carries a respawner pod and feedstock for a few bodies
  (assumption: 4; question 4). When the field lifeline is within reach of the craft, a sleeper can
  wake there at once and go back out. So reviving a friend mid-mission means carrying the field
  lifeline back to the craft: a trek with 40 kg on your back, which is exactly the effort his
  sentence asks for. When the feedstock runs out, the rest sleep until home.

The death screen lists where the sleeper can wake as each becomes possible (draft): "You are asleep
on the field lifeline. Wake at the craft when it is near, or wait for home."

### 2.6 A death out of its reach: the window

A person who dies out of reach had one support, their brain. **A dying brain holds its mind, dimmed,
for a short while** (the lore). In that window the carrier can bring the field lifeline within reach
of the body; the mind joins it while it is still running, and from then on the person is asleep on
the field lifeline (2.5). That is the revive: **someone bringing the heavy piece to you.**

- **Hardcore, your own kit: 5 minutes (assumption).** The real anchor: brain injury after cardiac
  arrest begins within about 4 to 6 minutes without CPR or defibrillation, and survival falls by
  about 7 to 10% for every minute without them (American Heart Association figures, read 2026-11-03
  through secondary sources that repeat them; check heart.org before quoting).
- **Realistic, crew issue: 12 hours (assumption).** The crew suit senses a death out of reach and
  cools the body. The real anchor: cold protects a brain. A woman whose circulation stopped in icy
  water, cooled to 13.7 °C, was revived and recovered (Gilbert and others, "Resuscitation from
  accidental hypothermia of 13.7 °C with circulatory arrest", *The Lancet* 355(9201):375-6,
  2000-01-29). Twelve hours is made up, and is long enough that the fleet's recovery run (2.8)
  always arrives in time.
- **Simplified:** cannot happen; the reach covers the mission.
- **Missed, in Hardcore:** the mind's last support stopped. The character ends
  (game-setting-and-intro.md 4.3: nobody is banned; the person arrives again as a new crew member).

**What is made up, said plainly** (for the Library's "What is made up in this game" page,
game-setting-and-intro.md 2.5): a real brain whose blood stops loses consciousness within seconds and
its activity stops; it does not keep a mind running slowly. Cold really does let some people be
revived after long arrests. The game's dying brain that holds a mind for minutes, or for hours when
cooled, is made up.

### 2.7 When its carrier dies, and when it is lost

- **The carrier dies.** The carrier is joined too, so they fall asleep on it like anyone. The field
  lifeline falls where they fell (it is the minds' support, so it is never re-issued and never goes
  into a death's pack). Anyone alive takes it up, and must take their backpack off to do it.
- **Nobody left alive.** Everyone sleeps on the field lifeline where it lies.
  - Simplified: it is aboard the craft (1.3), and the craft flies home by itself in about a minute
    (assumption).
  - Realistic: the fleet's recovery run brings it home (2.8).
  - Hardcore: its battery is the clock (assumption: 72 hours of real time, the setting design's
    recorder number). Other players can fly out to fetch it: a rescue mission, which is good content
    for a shared world.
- **Destroyed: Hardcore only.** A short list of causes in data, as the setting design had for the
  recorder: inside a star, a drive or reactor blast, burning up on entry, crushed in a giant planet's
  depths. Everyone asleep on it ends; everyone alive and joined to it is back on one support.
- **Out of power: Hardcore only.** The minds asleep on it end when it runs flat. It recharges at
  the craft and at home. Crew issue never runs flat.
- **Left behind.** It carries a locator, shown on the Maps page to its team (and in Realistic to the
  fleet's recovery run). In Hardcore, a field lifeline nobody can find is a battery running down.

### 2.8 How it is brought home

- **Carried into the lifeline**, or aboard a craft that flies back inside it: every mind on it
  rejoins the lifeline and wakes at its chosen place.
- **The fleet's recovery run (Realistic):** a field lifeline with no living carrier, and any body
  still in its window, is collected on a timetable (assumption: within an hour of real time, the
  setting design's number). The run carries its own field lifeline, so even the fleet follows his
  rule: someone brings the heavy piece.
- **By itself (Simplified):** the craft comes home in about a minute (2.7).
- **On a server**, the field lifeline is a small row on the relay: who carries it or where it lies,
  its battery, the minds joined and asleep on it. The recovery run is a sweep once a second, like
  the departures of ship-homes-and-logistics.md 5.10.

### 2.9 How it keeps the three rules

- **Never a copy.** It runs minds; it never records one. When nobody is joined it holds nothing at
  all. A mind joins it only while that mind is still running (alive, or dimmed in its window), never
  from a stored state. If the window closes first, that person has died, and nothing can bring them
  back: rule 3 is what makes a Hardcore end real.
- **Never two.** A pod grows a body only for a mind asleep on a support whose body has died. The
  server, which knows who is asleep on which field lifeline, refuses a second body (the setting
  design's "two of you" check, game-setting-and-intro.md 2.4).
- **Never a gap.** A death within reach dims the feed; a death out of reach dims it too, in the dying
  brain, until the field lifeline arrives or the window closes. The feed never cuts and restarts.

### 2.10 A team without one

- **Simplified and Realistic:** cannot happen. Every craft that leaves the lifeline carries one as
  crew issue. If nobody straps it on, it stays aboard and holds the minds within 200 m of the craft
  (Realistic); a death further out is the 12-hour window and a rescue.
- **Hardcore:** allowed, by choice. A team may leave it home to travel light: every backpack on,
  everyone faster. Then every death beyond the lifeline is final once its five minutes pass, unless
  another team's field lifeline reaches the body in time. The departure board says so before anyone
  boards, and the HUD says so out there. Nothing stops anyone going.

### 2.11 What it changes in game-setting-and-intro.md section 4

**The recorder every crew member wears (its 4.1) is replaced** by one field lifeline per team. The
recorder was a small personal unit that kept one mind; the field lifeline is a heavy shared one that
keeps a team's, which is the point of his (a): bringing someone back is somebody's work. The table in
its 4.2 becomes this one:

| | **Simplified** (the default) | **Realistic** | **Hardcore** (chosen per character) |
|---|---|---|---|
| **Inside the lifeline** | Wake at once at your chosen respawner. Nothing is lost. | Wake at once. Your backpack's contents lie in a pack where you fell, 60 minutes of play; what you wore is re-issued. | As Realistic. Inside the lifeline Hardcore costs nothing more. |
| **Beyond it, within a field lifeline's reach** | Wake at the craft's pod within a minute. Nothing is lost (section 3.6 says why this changes the setting design). | Asleep on the field lifeline until it reaches a pod: the craft's, or home. The pack lies where you fell; worn gear re-issued from the craft's stores. | As Realistic, plus Hardcore's costs (section 3): recommended, everything you had with you and the experience gained since you last slept. |
| **Beyond it, out of reach** | Cannot happen: the reach covers the mission. | The crew suit holds you for 12 hours. A teammate brings the field lifeline, or the recovery run does within the hour; then as the row above. | 5 minutes for someone to bring a field lifeline within reach of your body. Caught: as the row above. Missed: the character ends. |
| **The carrier dies** | (It rides aboard the craft.) If the whole team dies, the craft comes home by itself. | The carrier sleeps on it; anyone takes it up. Nobody left: the recovery run brings it home. | As Realistic, but nobody left means its 72-hour battery is the clock for a rescue. Destroyed or flat: everyone asleep on it ends. |
| **No field lifeline on the team** | Cannot happen. | Cannot happen. | Allowed and said plainly. Every death out there is final after its five minutes. |
| **What a death can cost** | a minute | the pack, and time asleep | the mission's gains; the character, if no support is left |
| **What the person always keeps** | everything they know | everything they know | their account, name, look, friends and standing, even when the character ends |

**What else in that design names the recorder**, to change when he accepts this (left as they are
now, so that design changes once, not twice): section 0's fourth bullet; the Fleet page's line
"Past it, carry a recorder, and go with friends" (1.1, recommended: "Past it, go as a team, and bring
the field lifeline"); the "Coming back as yourself" page's "unless you carry a recorder" (2.2); the
HUD and death-screen lines of 3.7; section 7's row "The link, beacons, recorders, the recovery run";
section 8's "Recovery runs and recorder clocks"; rung 8 of section 9 (now this design's rungs 4 to 6);
question 6 (the recorder's numbers, now questions 2 and 3 here); and Appendix A's flight-recorder row.
Its 4.3 option 3 (a skill or experience penalty, rejected there) is reopened by his (b), and section
3 here answers it without breaking his "They maintain all of their knowledge".

---

## 3. Hardcore's costs

Hardcore is chosen per character at the Arrivals desk, can be left at any time and never re-entered
(game-setting-and-intro.md 4.3). This section is what a Hardcore death costs. He called it brutal and
wants to keep playing with it, so every cost below is meant to be a switch in data (proposed
`data/world/hardcore.ron`), not a decision baked into code.

### 3.1 The options side by side

| | Option | What a Hardcore death costs | The lore | Keeps the three rules? | Keeps "all of their knowledge"? |
|---|---|---|---|---|---|
| **A** | The character ends (the setting design's 4.3) | the character, but only when no support is left: out of reach and the window closed, or the field lifeline destroyed or flat with you asleep on it | rule 3: nothing can wake a mind that stopped | yes | yes |
| **B** | Everything you had with you (his first idea, the gear half) | everything carried AND worn, not only the backpack. **B1:** gone. **B2:** in one pack where you fell, for your team to carry home if they can | his own rule, "lost everything physical"; on your own kit nobody re-issues anything | yes | yes |
| **C** | The experience gained on the mission (his first idea, the experience half) | the skill experience gained since you went out. **C1:** since you left the lifeline. **C2:** since you left the lifeline or last slept, whichever is later | C1: experience is not settled until you come home. C2: new learning settles over hours and in sleep, and your own kit does not hold it while it settles (3.2) | yes: you remember the mission; what is lost is the skill it had not yet built | yes, if the lore says what was lost was not yet knowledge |
| **D** | The highest skill's last full level (his second idea) | the experience of one full level of your highest skill. **D1:** any skill, as he said. **D2:** the highest skill that lives partly in the body. **D3:** every skill that lives partly in the body works lower in a new body until practice brings it back | the body's share: some of a skill is strength, endurance, calluses and the body's fit to the work, which a body grown fresh from your genes has never had | yes | D1 no, for skills that are all knowledge; D2 and D3 yes |
| **E** | Combinations | A+B+C (recommended, 3.5); A+D; A+B+C+D (everything) | each part's own | yes | as their parts |

### 3.2 The lore for each, without breaking "never a gap"

"Never a gap" promises that the feed never cuts: you remember falling asleep, and you remember
everything. So no cost here may take a memory. Each one takes something else, and says what.

**B, things.** Already the setting's lore: a new body is grown with nothing physical. Realistic
re-issues worn gear from the fleet's stores; Hardcore, on its own kit, has no stores out there but
the crew basics the craft carries. B2 keeps the things in the world, which is real (nothing
vanishes), and uses the pack that exists (death-and-your-pack.md), with worn gear added to it. On a
mission beyond the lifeline, carrying a friend's pack home is a real choice, because the carrier's
back is already full.

**C, experience not yet settled.** Two lores:
- **C1, "not settled until you come home".** Pure fiction: the fleet's lifeline settles what you
  learn, and out past it the newest learning lives in your brain alone. Simple, and exactly his
  scope ("during the mission"). It teaches nothing about real learning, true or false.
- **C2, "settles over hours and in sleep"** (recommended). The real part: a brain settles new
  learning over the hours after it and especially in sleep. That is memory consolidation (Diekelmann
  and Born, "The memory function of sleep", *Nature Reviews Neuroscience* 11(2):114-126, 2010-02,
  a review). And knowing that something happened is a different memory from a skill gained by doing
  it: the patient known as H.M. got better at mirror drawing across days without remembering ever
  having practised (Brenda Milner, 1962). The made-up part: the fleet's crew issue holds new learning
  while it settles, your own kit does not, and a death before it settles loses the skill it had not
  yet built. So you remember the whole mission and lose its gain in skill. It also gives the field a
  use for sleep: make camp, build a shelter and a fire, sleep the night, and what you learned is
  yours. Coming home settles it too.

**D, the body's share.** The real part: a body keeps a record of its training. Muscle fibres gain
nuclei when they are loaded, and keep them long after the muscle shrinks: in mice, for at least
three months after its nerve was cut (Bruusgaard and others, "Myonuclei acquired by overload
exercise precede hypertrophy and are not lost on detraining", *PNAS* 107(34):15111-6, 2010-08-24).
Strength, endurance and calluses are the body's. A body grown fresh from your genes has had none of
it. The technique itself, how to swing the hammer, is learned in the brain, so it is the mind's and
is kept. Where D's lore breaks:
1. **Skills that are all knowledge.** Leadership, Trading, Tactics, Navigation, Engineering and
   Medicine (data/skills/skills.csv) have almost no body in them. D1, "just any skill, whichever is
   highest", would take a level of Leadership for a reason that is not true, and would teach that a
   new body forgets how to lead. That is also the one cost here that contradicts his own "They
   maintain all of their knowledge". D2 avoids it with one column in skills.csv saying how much of
   each skill lives in the body.
2. **It compounds; the body does not.** A new body is out of condition once, the same after every
   death: the real cost is getting back in condition. D1 and D2 take another level each time. D3 is
   the true-to-the-body version: temporary, never compounding, and regained by practice. It needs a
   conditioning system the game does not have yet (data/aging_fitness.ron authors six fitness levels
   from Sedentary to Elite, and its AgingSystem is deferred in tests/engine_wiring_lint.rs).

### 3.3 "The previous full level", in today's code

Skills are kept as a level and the experience earned toward the next level
(crates/humanity-systems/src/skills/mod.rs, `SkillProgress { level, xp }`; leftover experience
carries over on a level-up). The experience to go from level L-1 to L is `xp_per_level` times L to
the power 1.5 (`SkillDef::xp_for_level`).

Taking the previous full level's worth from a skill at level L with x experience toward L+1 leaves it
at level L-1 with the same x, or at level L with x less that level's worth when x was already that
large. **In words: you drop one level and keep your progress toward the next.** (Appendix B.)

- **What it is worth** (computed): Mining (`xp_per_level` 100) at level 10 loses 3,162 of the 14,264
  experience it took to get there, 22%. A mining drone's haul trains Mining at 1 experience per ore
  unit (crates/humanity-systems/src/mining.rs), so that is 3,162 ore. At level 20 it is 8,944 of
  76,072 (12%); at level 5, 1,118 of 2,819 (40%). The curve steepens, so the share falls as you rise.
- **"Whichever is highest"** needs a tie rule: the higher experience toward the next level, then the
  order of skills.csv, so the same save always loses the same skill.
- **Edges.** A skill at level 0 loses nothing. A skill at its `max_level` stops gaining experience
  (`SkillSystem::tick`), so it drops to the level below with nothing toward the next.
- **Rolled back, not deducted, for C.** C restores the skills as they were at the last settling
  (a copy of `WorldSave::skills` taken on leaving the lifeline and at each sleep): levels gained since
  go too. Quest rewards count as experience like any other.
- **Who enforces it.** On a server with closed or hybrid custody
  ([characters-and-servers.md](characters-and-servers.md)) the server holds the settling copy and
  applies the cost; on an open server and solo the save is the player's own, so Hardcore is the
  player's word there, as the setting design already says (4.3).

### 3.4 Which mode each fits, and what each does to the fun and to teaching

| Option | Simplified | Realistic | Hardcore | The fun | The teaching |
|---|---|---|---|---|---|
| A, the character ends | never | never | always: it is what Hardcore means | the highest stake there is; a story a server remembers | care and preparation; the real rule never to go into the wild alone |
| B, everything you had with you | never (his rule: nothing lost) | the backpack only, as built | recommended, as B2 | push your luck; carry what you can afford to lose | pack light, pack what you need |
| C, the mission's experience | never | never | recommended, as C2 | tension on every mission; a reason to come home, or to make camp and sleep | real: sleep settles what you learn, and rest is part of learning |
| D, the highest skill's level | never | D3 later, if he wants realism there, once conditioning exists | a dial, off by default; D2 when on | fear for your best skill; a push to spread out | D1 false for knowledge skills; D2 and D3 true to the body |

### 3.5 Recommendation (his to keep playing with)

**Hardcore = A + B2 + C2.**
- A death beyond the lifeline that is caught (within a field lifeline's reach, or brought into it in
  the window) costs **everything you had with you**, in one pack where you fell, and **the experience
  gained since you left the lifeline or last slept**.
- A death with no support left **ends the character**, with no ban (the setting design's 4.3).
- Inside the lifeline, Hardcore costs what Realistic costs, as the setting design says.

Why this one:
1. **It is his first idea, whole,** with sleep added as the way to bank what you learn: "losing all
   gear and experience gained during the mission".
2. **It keeps every rule.** Nothing copies, nothing doubles, and no memory is taken; what is lost is
   things and skill that had not settled, and his "They maintain all of their knowledge" stands.
3. **It puts the danger where the setting puts it,** beyond the lifeline, and makes coming home mean
   something. It also gives the field lifeline a job in Hardcore even when nobody's life is at stake:
   a caught death is still costly, an uncaught one is final.
4. **It teaches something true** (sleep settles learning) and nothing false.

**Why D stays a dial, off:** it falls hardest on the players who invested most, it compounds where a
body would not, and in his exact form (D1) it takes knowledge, which is the one thing his own
setting says a death never takes. If he turns it on, D2 (the highest skill that lives partly in the
body) keeps it honest. D3 is the realistic version for later. Question 6.

### 3.6 Simplified and Realistic beyond the lifeline

- **Simplified loses nothing anywhere,** beyond the lifeline included. The setting design's 4.2
  charged a Simplified death beyond the link "what a Realistic death costs today" (the pack). That
  contradicts his 2026-10-04 rule for Simplified, "Nothing was lost". Recommended: his rule
  (question 5).
- **Realistic beyond the lifeline** costs what Realistic costs at home (the pack where you fell, worn
  gear re-issued, here from the craft's stores) plus the time asleep. It takes no experience:
  Realistic's realism is about things and time, and experience is Hardcore's stake.

---

## 4. Magic

He wrote "maybe revives use magic, since I kinda want to include magic". This section gives him the
honest options and what each costs, held against [two-realities.md](two-realities.md): "we teach
real-life skills by mirroring them in the game", so whatever the game teaches as real must stay true.

### 4.1 What exists today (read from the code)

Magic is already in the game, and most people working on it do not know.

- **data/abilities.csv** has 110 rows: 94 with the flavor `fantasy`, 10 `tech` and 6 `real`
  (counted). The fantasy rows sit in eleven schools (pyromancy, cryomancy, geomancy, aeromancy,
  druidism, shadow_arts, divinity, arcana, summoning, enchanting, psionics), none of which is one of
  the 20 skills in data/skills/skills.csv.
- **The `flavor` column filters nothing.** It is parsed (`AbilityDef::flavor`,
  crates/humanity-systems/src/abilities.rs) and printed on the Profile page's ability grid as
  "school | flavor" (src/gui/pages/profile.rs), and that is all. The file's own header still says
  "Real mode shows real+tech", but there has been no Real mode since 2026-07-16 (two-realities.md:
  "No Real/Sim toggle").
- **Fourteen spells are castable by every player in every world right now.** A skill gate at level
  1 or below is open to everyone (`AbilityDef::skill_gate_met`: "Level-1 gates are
  baseline-open"), and the abilities bridge (src/lib.rs, `state.gui_state.abilities`) puts every
  castable row on the HUD's 1 to 9 bar. The open fantasy rows with an effect: Fireball, Ember Shot,
  Ice Bolt, Rock Throw, Lightning Bolt, Gust, Shadow Bolt, Smite, Magic Missile and Telekinesis
  (damage, cast at a creature), and Cauterize, Heal, Purify and Holy Light (healing, cast on
  yourself). Nothing on screen says they are magic beyond the word "fantasy" on the Profile grid.
- **A `resurrect` row exists** ("Bring a fallen ally back to life", school light, skill divinity at
  level 8). It is locked, because divinity is not a skill, and does nothing (no damage, no healing:
  the bridge's "Does nothing yet - a later arc wires it").
- **The relay already separates the two kinds of experience:** `reality_xp` and `fantasy_xp` per
  skill per person (src/relay/storage/skill_dna.rs, `user_skills`).
- **The intent was always a filter:** progression-skills-gear.md, pillar 4, "fantasy-flavored
  content (spells) simply does not exist in Real mode's data view".

### 4.2 The options, and what each costs

**M1. Magic as the lore's explanation for revival.** The lifeline and the field lifeline work by
magic, or by the soul or "source energy" (his words of 2026-11-02 name soul, mind, body and source
energy).
- *Gives:* the magic he wants, at the heart of the setting; his own words on screen.
- *Costs:* it takes a side on the soul. The setting design (1.2) recommends that "The link carries
  your mind. What else a person is, the fleet leaves to each person", because a game for billions of
  people should not settle that question; a revival by the soul tells every player what a soul
  does. It puts magic at the root of the one fiction a teaching game most needs to keep labelled, in
  a world whose Library teaches the real body. It changes no mechanic, only the word, and "it is
  magic" explains less than "it is made up, and here is the real science beside it". And Hardcore
  needs a reason the soul does not carry a mind whose last support stopped, which rule 3 now gives
  for free.

**M2. "Magic" as the crew's word for what nobody aboard understands.** The fleet's world stays
technology. What the crew find and cannot explain, they call magic: above all the devices of the
makers of Agartha, the artificial world at the end of the main story (the film's canon,
game-setting-and-intro.md 1.2). This is Arthur C. Clarke's third law turned into a setting rule:
technology far enough ahead of you cannot be told apart from magic (*Profiles of the Future*,
revised edition, 1973, paraphrased).
- *Gives:* mystery and a hook for the story; the word "magic" on screen; and a true lesson, that
  what one age called magic a later one explained.
- *Costs:* no spells in the fleet's own world. Found "magic" must stay rare and unexplained (a story
  budget), and must never reach a system that teaches.

**M3. Magic as its own clearly fictional layer: a world setting.** A world (a server, or a solo
world) chooses "Magic: on" and the fantasy rows load there: spells, schools, magic revival. The
fleet's world is untouched.
- *Gives:* the full fantasy he "kinda" wants, and a use for the 94 authored rows.
- *Costs:* a second lore. Recommended: a world of magic keeps the three rules and his heavy-piece
  rule, so revival there is a spell that holds a fallen friend's mind the way the field lifeline
  does, cast by someone carrying a heavy focus on every back slot (draft words; the authored
  `resurrect` row becomes that). Eleven schools that are not skills: the 20-skill budget
  (progression-skills-gear.md, "Keep exactly 20 skills") holds for the fleet's world, and the schools
  are a separate list that only magic worlds load. A filter wherever abilities are listed, and the
  marks of 4.4 wherever magic could meet something real.

**M4. Magic as a game the crew play** (a small bridge, optional). In the fleet's own world, spells
work only in the Arena and the play halls, as sport between people who both chose it: the same
answer he agreed for weapons (game-setting-and-intro.md 10.1).
- *Gives:* magic inside the main world without touching its lore.
- *Costs:* the filter by place as well as by world, and the Arena as a real place (it is a labelled
  district today, data/blueprints/ship_structure.ron).

### 4.3 Recommendation

**M2 for the fleet's world, M3 as a world setting, and M4 if he wants spells inside the main
world. Not M1.** Revival in the fleet's world stays the field lifeline, labelled made up beside the
real science. And close today's gap first: the flavor column becomes the filter it was meant to be,
magic is off in the fleet's world until he says otherwise, and Dev mode still shows everything
(rung 2, question 8 and 9).

### 4.4 Where magic may and may not appear

| Place | Magic? |
|---|---|
| A world that turned magic on | yes, marked (4.5) |
| The Arena and play halls (M4, if chosen) | yes, by consent |
| Story places such as Agartha (M2) | the crew's word "magic" for things marked as unexplained |
| Creatures | spells strike creatures, as casting does today (a faced creature in range); never people, except by consent in the Arena |
| Crops and the garden (data/plants.csv, the farming numbers) | never in the fleet's world. In a magic world a spell's effect on a crop is marked on that crop and never reaches the numbers the Library, the Real Skills curriculum or the Home page's self-sufficiency score show |
| Medicine and first aid (data/medical/, the First Aid ability, wounds) | never a spell named or shaped like a real treatment; a magic heal is marked and never offered as the way a real wound is treated in a lesson |
| The homestead's numbers: power, water, air, food | no magic source in the fleet's world; in a magic world a magic source is marked and kept apart in every readout |
| The Library, the curriculum, how-to pages, the glossary | only on "What is made up in this game" and a magic world's own page |
| The real side of the app (Home, Real Me, Tasks, Market, Chat) | never |
| Experience | fantasy rows train `fantasy_xp` only, never `reality_xp` |
| Revival in the fleet's world | never magic: the field lifeline |

### 4.5 How a player always knows which is which

1. **A world mark.** While magic is on, the HUD paints MAGIC, as it paints CREATIVE or DEV today
   when the powers mode is not Normal (src/gui/pages/hud.rs; game-modes.md section 1), so a
   screenshot cannot pass a magic world off as the real one.
2. **A thing mark.** Every fantasy ability, item, effect and creature carries the magic mark on its
   tile, card and tooltip, and its words say "magic" plainly (words from `data/i18n/en.json` by key;
   the mark from the glyphs `icon_glyph_lint` allows).
3. **One page.** The Library's "What is made up in this game" (game-setting-and-intro.md 2.5) lists
   magic beside the lifeline, the field lifeline, the respawner's speed, the jump, the cold-sleep
   suit and the dying brain's window (2.6).
4. **Tests that hold the line.** A lint refuses a fantasy row gated on one of the 20 real skills, a
   fantasy row whose effect touches crops, water, air, power or a real treatment, and fantasy
   experience reaching `reality_xp`. The `tech` rows (Nano Heal, Energy Shield) are the fleet's own
   fiction, so they carry the "made up" note wherever they could be mistaken for real.

---

## 5. Everything respawning costs and gives

His "infinitely respawning lore mechanics" in one place: what coming back costs and gives, by mode,
with the recommendations of sections 2 and 3 applied. Every death can repeat forever; nothing here
runs out, except a Hardcore character.

### 5.1 The table

| What | Simplified | Realistic | Hardcore | The rule it rests on |
|---|---|---|---|---|
| Knowledge: skills, quests, recipes, what was learned | kept | kept | kept | the mind (1.2) |
| Memories, the death and the dark time included | kept | kept | kept | never a gap |
| Experience gained beyond the lifeline since you last slept | kept | kept | lost on a caught death beyond the lifeline (C2) | not yet settled, on your own kit (3.2) |
| The highest skill's last full level | kept | kept | kept, unless he turns the dial on (D2) | the body's share (3.2) |
| What you carried | kept: the fleet brings it back with you | one pack where you fell, 60 minutes of play | inside the lifeline as Realistic; beyond it, in one pack with what you wore (B2) | everything physical is lost |
| What you wore | kept | re-issued: from the fleet's stores at home, the craft's out there | inside as Realistic; beyond, in the pack, and you wake in crew basics | everything physical is lost |
| Body mods (implants, augments, genetic changes), wounds, illness, scars | gone | gone | gone | a new body has only your genes |
| Your look | kept | kept | kept | his 2026-10-04 rule, "only their appearance is kept" |
| The body | one `body` line in the fleet ledger, never refused | the same | the same | the setting design's 2.4 |
| Time | at once inside the lifeline; about a minute beyond it | at once inside; beyond it, asleep until the field lifeline reaches a pod, which the team decides, or within the recovery run's hour when nobody is left to carry it | as Realistic, or the end | the cost is what a death loses, never a wait for its own sake |
| Where you wake | your chosen respawner; the craft's pod beyond the lifeline | the same, when the field lifeline reaches one | the same | respawners as places (setting 5.6) |
| The character | kept | kept | ends only when no support is left | never a copy |
| The person: account, name, look, friends, standing | kept | kept | kept, even when the character ends | nobody is banned for dying (setting 4.3) |
| The home | kept | kept | kept; if the character ends, to the named heir or the fleet | the setting design's 4.3 |
| What a new body gives | full health, food, water and rest raised to at least 60, effects cleared (built since v0.745) | the same | the same | a healthy body grown from your genes |

### 5.2 Options that would break a rule, and what that would cost

- **Waking from a backup** (a copy). Already rejected (setting 4.3, option 4). Cost: the teleporter
  question comes back, and Hardcore could never be final without a ban, because a backup could always
  be restored.
- **A field lifeline that stores minds and wakes them later** (a copy). So it must run minds and never
  record one, and a Hardcore one that runs flat ends the minds on it. Cost of breaking it: the same.
- **Taking memories:** forgetting the mission or the moment of death (a gap). So C takes skill that
  had not settled, never memories. Cost of breaking it: "You remember falling asleep. You remember
  everything" (the Library page drafted in setting 2.2) becomes false.
- **A second body while the first lives,** or two pods growing one each (two of you). The server
  holds who is asleep on which support and refuses a second body.
- **Magic revival from nothing** (a copy, unless the spell holds a mind that is still running). M3's
  rule (4.2) keeps magic worlds inside the three rules.
- **Bringing an ended Hardcore character back by any means** (a copy). On closed and hybrid custody
  the server refuses it; on open servers and solo, Hardcore is the player's word (3.3).

### 5.3 One more way back, entirely real: first aid before death (an option)

His rule is about the dead. Real medicine has a moment before that: when a heart stops, the person
can still be saved for a few minutes, by CPR and a defibrillator, in the same body. Brain injury
begins within about 4 to 6 minutes without them, and survival falls by about 7 to 10% a minute (the
American Heart Association figures of 2.6). A **collapse** before death would let a teammate save
you with real first aid: no respawn, no new body, nothing lost, and the most useful skill a game
could teach. It does not contradict his rule if "dead" means past saving, and it needs a state the
game does not have (a body that is down but not yet dead). Question 10.

---

## 6. Accessibility and the no-fail path

game-modes.md section E: a no-fail path is an accessibility requirement (children, disabled players,
anyone using HumanityOS as a life tool). Today it is Simplified death ("Nothing was lost"), plus the
vitals drain at 0 and hostile wildlife off. This design keeps it whole, and says what each option
does to it.

- **Simplified stays no-fail everywhere.** It loses nothing, beyond the lifeline included (3.6). It
  never asks anyone to carry the field lifeline (the craft carries it and its reach covers the
  mission), has no window and no final death, and wakes you within a minute.
- **The field lifeline (Realistic, Hardcore).** The weight is two settings, not one: in Forgiving
  carrying (the default) 40 kg warns and slows nobody. It can be set down, and it changes hands with
  one held press. Realistic's 12-hour window means a rescue is never a race.
- **Hardcore's costs (A to D)** exist only for a character that chose Hardcore at the desk, and can be
  left at any time (setting 4.3). The desk never suggests it; for children it suggests the Cabin and
  Simplified together (setting 5.4).
- **Magic** changes nothing about failing. Its marks are words as well as a glyph, so a screen reader
  says "magic", and never colour alone.
- **The reach** shows as a ring on the ground, as words on the HUD ("3 of 4 in reach", a draft) and as
  a sound at its edge, never by colour alone.
- **The wait asleep** can be signed off from, slept through in a solo game, and is about a minute in
  Simplified.
- **The dual-mode rule** (CLAUDE.md, "Dual modes"): every deep system here ships with its simplified
  mode. The field lifeline has Simplified's craft-borne cover; carrying has Forgiving; a conditioning
  system (D3), if built, would ship with a simplified mode too.

---

## 7. Budgets

Per CLAUDE.md, each effect states its per-frame budget and its network footprint before it is built.
The target is his display: 2560x1440 at 120 fps, 8.3 ms a frame. Budgets to measure against, not
measurements.

| Effect | Runs where | Per-frame budget | Network |
|---|---|---|---|
| Who is in reach (each joined person against each field lifeline, f64) | the server once a second; each machine once a second for the HUD | 0.01 ms | positions already stream; one small note when someone leaves or enters reach |
| The reach ring and the HUD's reach line | each machine | GPU 0.1 ms, CPU 0.05 ms | none |
| Asleep on the field lifeline (the dim view) | each machine, a post-process | 0.3 ms, the setting design's budget | voice as today |
| The window's countdown and the recovery run | the server, a sweep once a second | nothing per frame | one message when a mind is caught, wakes or ends |
| The field lifeline's row (carrier or place, battery, minds joined and asleep) | the server | nothing per frame | a few hundred bytes on change, to its team |
| The settling copy for C2 | the save, and the server on closed custody | nothing per frame: a copy of 20 skills at departure and at each sleep | on closed custody, one message per settling |
| The magic filter | at load and when the setting changes | nothing per frame | the world's magic setting in the welcome, a few bytes |
| The MAGIC mark | inside the HUD's existing mode tag | nothing extra | none |

Everything cosmetic (the ring, the dim view, the ride home) is drawn on each machine and never
networked; only gameplay state (who carries what, who is asleep on what, a window's end) syncs, at a
low rate.

---

## 8. What exists in code, and the rungs

### 8.1 What exists

| Piece | Exists today (where) | New in this design |
|---|---|---|
| Death, the death screen, Respawn | since v0.745: src/gui/pages/hud.rs `draw_death_screen`; src/lib.rs `pending_respawn`; rig verbs `{"die":...}` and `{"respawn":"1"}` (src/engine/ipc.rs) | the field lifeline's lines; where you can wake, as each becomes possible |
| The death modes | Simplified and Realistic, `DeathMode` (crates/humanity-systems/src/death_pack.rs); a per-install switch, `AppConfig::death_realistic` (src/config.rs) | Hardcore per character, in the save |
| The pack where you fell | `LeftPack`, `on_death` (backpack only; the `Outfit` is not touched); data/world/death.ron; src/engine/death_pack.rs | worn gear into the pack for Hardcore beyond the lifeline (B2) |
| Skills and experience | crates/humanity-systems/src/skills/mod.rs (`SkillProgress { level, xp }`, `xp_for_level`, the `xp_grants` channel); data/skills/skills.csv (20 skills); saved in `WorldSave::skills` (src/save_load.rs) | the settling copy (C2); the highest-skill loss and `body_share` (D2), if chosen |
| The back slot | one `back` slot (data/inventory/equipment_slots.json); `Outfit::equipped`; backpacks add carry (data/equipment.csv) | "all back slots"; the field lifeline's rows in items.csv and equipment.csv |
| Carrying weight | crates/humanity-systems/src/encumbrance.rs (the limit follows gravity; Forgiving and Realistic); the weight tally in `InventorySystem::tick` | worn mass counts; the volume follows the back slot |
| Fitness | data/aging_fitness.ron (six fitness levels, authored); AgingSystem deferred (tests/engine_wiring_lint.rs) | conditioning, for D3, later |
| Abilities and magic | data/abilities.csv (94 fantasy rows, the `flavor` column unread); crates/humanity-systems/src/abilities.rs; the HUD's 1 to 9 bar; the relay's `reality_xp` and `fantasy_xp` | the magic world setting, the filter, the marks, the lints |
| Sleep and beds | built beds work (src/engine/built_uses.rs); sleep runs the clock fast (crates/humanity-systems/src/time.rs) | a sleep settles experience (C2) |
| Custody | designed, partly pre-built server-side ([characters-and-servers.md](characters-and-servers.md)) | the server holds Hardcore's costs on closed custody |
| The lifeline, beacons, missions, the jump, mission craft | none (game-setting-and-intro.md section 7) | the field lifeline, the craft's pod, the recovery run |

### 8.2 The rungs, smallest first

Each rung is playable and a rung of the real thing (the realistic-first rule).

0. **His answers** to section 9. Gate: his review of this document.
1. **The words.** Glossary terms (field lifeline, reach, asleep on the field lifeline, crew issue,
   own kit, magic); the Library pages "Coming back as yourself" (the field lifeline in place of the
   recorder) and "What is made up in this game" (2.6, 4.5); the desk's line of 1.3; the setting
   design's recorder lines (2.11). Gate: `node scripts/check-doc-links.js` at 0, the Library freshness
   check, the glossary and i18n tests, the terminology lint; he reads the pages.
2. **Magic labelled and gated.** A world setting, Magic: off, in the Arena, or everywhere (Server
   Settings and the solo world's settings, both apps); the `flavor` column filters the registry's
   view, so fantasy rows leave the HUD bar and cannot be cast where magic is off; Dev mode shows all;
   the MAGIC mark and the thing mark; fantasy experience to `fantasy_xp`; the lints of 4.5. Needs
   nothing else. Gate: lib tests (magic off: Fireball is not on the bar and a cast is refused; on:
   castable; seen red with the filter taken out); a snapshot of the Profile's abilities with the mark;
   the i18n and glyph lints.
3. **The back and its weight.** Worn gear's own mass counts toward the load; the carried volume
   follows the back slot; the field lifeline as an item (data/items.csv, 40 kg) and as gear taking
   all back slots (data/equipment.csv); handing it over; setting it down. Gate: lib tests of the
   carry numbers at 1 g, on Mars and on the Moon and weightless (Appendix B), each seen red; the
   encumbrance tests still pass; a snapshot of the Inventory page with it worn.
4. **Its minds, beyond the lifeline, before the jump.** Needs the setting design's rung 5 (the
   lifeline's inside test in f64) and two dev verbs, proposed `{"lifeline":"off"}` and
   `{"lifeline":"on"}`, which stand a rig beyond the lifeline without any craft. The roster, the
   reach test, the ring and the HUD line, asleep on the field lifeline, the window by mode, waking
   when it comes inside the lifeline, the carrier's death and taking it up, Simplified's cover, the
   death screen's lines. Gate: a lib test for every cell of the table in 2.11; a rig that dies out of
   reach, carries the field lifeline to the body inside the window, then wakes on
   `{"lifeline":"on"}`, and one where the window closes in Hardcore; each seen red.
5. **Hardcore per character, and its costs.** The save field (chosen at the desk of the setting
   design's rung 3; until then in Settings > Gameplay > Death for this character, leave-only);
   B2's pack; C2's settling copy, taken on leaving the lifeline and at each sleep; A's end, held by
   the server on closed and hybrid custody; D as an off dial if he wants it. Every cost a row in
   `data/world/hardcore.ron`. Gate: lib tests (the settling copy and its rollback; levels gained on
   the mission go; a sleep banks; coming inside banks; D's arithmetic and tie rule if on); relay tests
   (the server, not the client, applies the cost; an ended character cannot join as itself).
6. **Mission craft.** Crew issue aboard every craft that leaves the lifeline; the craft's pod and its
   bodies; the recovery run; the Hardcore battery and the list of causes that destroy a field
   lifeline; the departure board's line and warning. Needs the jump and missions (the setting
   design's rung 8). Gate: relay tests (the recovery run collects within its time; a flat battery ends
   the minds on it; a second body is refused); a rig per mode with scripted teammates.
7. **Down before dead, if he says yes** (5.3). A collapse state, a teammate's first aid, the real
   numbers in data with their sources. Gate: lib tests of the collapse window; the Library's first-aid
   page and the quest that teaches it cite their sources.
8. **Revival in a world of magic, if he chooses M3.** The `resurrect` row wired under the three rules
   and his heavy-piece rule (the focus on every back slot). Needs rungs 2 and 4. Gate: lib tests that
   it holds only a still-running mind and never a second body.

---

## 9. Questions only the operator can answer

Each has a recommendation.

1. **The heavy piece's name.** Recommend "the field lifeline": it says what it does in the word you
   agreed for the link. Not "lifeline pack" ("your pack" is what a Realistic death leaves) and not
   "recorder" (it reads as a copy).
2. **Its numbers.** Recommend, all data: 40 kg; every back slot; reach 200 m; 12 minds; a 72-hour
   battery in Hardcore only. Heavier is possible, but 54 kg is about the most an army plans anyone to
   carry, and a literal tonne no one can move at Earth's weight.
3. **The window.** Recommend 5 minutes in Hardcore (the real figure for a brain without CPR) and 12
   hours in Realistic (the crew suit's cold). The alternative, no window at all, makes only deaths
   inside the reach catchable, and loses the rescue that makes "someone bringing the heavy piece"
   a story.
4. **Do mission craft carry a respawner pod?** Recommend yes, with feedstock for 4 bodies, so a friend
   can be woken mid-mission by carrying the field lifeline back to the craft. Without it, every death
   out there waits for home.
5. **Simplified beyond the lifeline.** Recommend that it loses nothing, your 2026-10-04 rule ("Nothing
   was lost"), instead of the setting design's pack where you fell.
6. **Hardcore's costs.** Recommend A + B2 + C2: a caught death beyond the lifeline costs everything you
   had with you and the experience since you left the lifeline or last slept; the character ends only
   when no support is left. The highest skill's last level (D) as a dial, off. All of it rows in data,
   to keep playing with.
7. **If the highest-skill loss is on: any skill, or only skills that live partly in the body?**
   Recommend the body's skills only (D2), because for a skill that is all knowledge, such as
   Leadership, the loss would take knowledge, which your setting says a death never takes. The
   temporary "out of condition" (D3) is the realistic form for when conditioning exists.
8. **Magic.** Recommend M2 for the fleet's world ("magic" is the crew's word for what nobody aboard
   understands, above all Agartha's makers), M3 as a world setting for spells and magic revival, and
   M4 (spells as Arena sport by consent) if you want them in the main world. Not M1, which would make
   revival a claim about the soul.
9. **Magic today.** Fourteen spells are castable by every player in every world now, unlabelled.
   Recommend magic off in the fleet's world from rung 2, on in any world that chooses it, and Dev mode
   showing everything.
10. **Down before dead** (5.3). Recommend yes, as a later rung: the one way back that is wholly real,
    and it teaches CPR.
11. **A Hardcore pack in a shared world.** Who may take from the pack you left beyond the lifeline?
    Recommend your mission's roster and nobody else, so a team can carry a friend's things home and
    nobody can loot a stranger (the shared pack needs the relay to hold it, death-and-your-pack.md
    "On a shared server").
12. **A names pass over the fantasy abilities.** Recommend yes, before magic worlds ship: several read
    as the spell names of well-known commercial games, and "Summon Undead" ("Raise the dead to serve
    you") turns the dead into servants in a setting where the dead are sleeping crew (Appendix C).

---

## Appendix A. Forms, and what is ours

Original work only (game-setting-and-intro.md Appendix A): no names, lore or mechanics copied from
another game. This design takes general forms, as the setting design did:

| Form | Where it is common | What is ours |
|---|---|---|
| A teammate brings a fallen player back | co-op games generally | the field lifeline, its reach and window, waking at a pod; the rule that it runs minds and never records them |
| One player carries the heavy thing the team depends on, and can carry little else | co-op games generally | every back slot; the backpack left behind; weight that follows real gravity |
| A death that costs experience | role-playing games since their start | experience not yet settled, banked by sleep; the body's share |
| A character that can end, while the player goes on | games with a hardcore mode | the end only when no support is left; a new crew member on the next transport; no ban |
| Magic as schools of spells | fantasy games generally | magic as a world setting with marks, kept out of everything the game teaches |

Arthur C. Clarke's third law, consolidation research, muscle research, cold resuscitation, CPR
figures and army load limits are cited as real thought and real science, not as borrowed fiction.

## Appendix B. Sources and computations

**Carrying** (crates/humanity-systems/src/encumbrance.rs: 50 kg comfortable at 1 g, 9.81 m/s2; the
limit scales by 9.81 / g; walking at 2 minus the load's ratio to the limit past it, never below 15%;
the jump's launch speed by the square root of 70 / (70 + load)):
- With the field lifeline (40 kg) and no backpack, at 1 g: 50 - 40 = 10 kg to spare.
- Carrying 20 kg more: 60 / 50 = 1.2 of the limit, so 2 - 1.2 = 80% walking speed.
- Mars (3.72 m/s2): 50 x 9.81 / 3.72 = 131.9 kg. The Moon (1.62 m/s2): 50 x 9.81 / 1.62 = 302.8 kg.
- The jump with 40 kg: sqrt(70 / 110) = 0.798; with 50 kg: sqrt(70 / 120) = 0.764.
- A tonne at 1 g: 1,000 / 50 = 20 times the comfortable limit; on the Moon 1,000 / 302.8 = 3.3 times.
- Army loads: 72 lb x 0.4536 = 32.7 kg; 120 lb = 54.4 kg; the fighting load's 48 lb = 21.8 kg.

**Reach:** a circle of 200 m: pi x 200^2 = 125,664 m2 = 12.6 hectares.

**Experience** (crates/humanity-systems/src/skills/mod.rs: the step from level k-1 to k costs
floor(base x k^1.5)):
- Losing the step to level L from (level L, x toward L+1): the total falls from
  sum(k = 1..L) + x to sum(k = 1..L-1) + x, which is level L-1 with x when x is below the step to L,
  and otherwise level L with x minus that step.
- Mining, base 100: the step to 10 is 3,162 and the total to 10 is 14,264 (22.2%); the step to 20 is
  8,944 of 76,072 (11.8%); the step to 5 is 1,118 of 2,819 (39.7%). Any base gives the same shares
  at the same level.

**Sources** (each with its own date):
- US Army FM 21-18, *Foot Marches*, 1990-06-01, superseded by ATP 3-21.18, 2017-04-17: fighting load
  48 lb, approach march load 72 lb, emergency loads to about 120 lb. Read 2026-11-03 through secondary
  summaries (chiefly a USMC combat load report of 2003 that quotes the manual), not the manual itself;
  check the current ATP before quoting.
- American Heart Association figures: brain injury begins in about 4 to 6 minutes of cardiac arrest
  without CPR or defibrillation; survival falls about 7 to 10% a minute. Read 2026-11-03 through older
  secondary sources that repeat them (a US Coast Guard safety alert of 2011 and its auxiliary's CPR
  sheet); check heart.org before quoting.
- Gilbert, Busund, Skagseth, Nilsen and Solbø, "Resuscitation from accidental hypothermia of 13.7 °C
  with circulatory arrest", *The Lancet* 355(9201):375-6, 2000-01-29.
- Diekelmann and Born, "The memory function of sleep", *Nature Reviews Neuroscience* 11(2):114-126,
  2010-02 (online 2010-01-04).
- Milner, the mirror-drawing study of the patient H.M., 1962 (in *Physiologie de l'hippocampe*,
  CNRS, Paris).
- Bruusgaard, Johansen, Egner, Rana and Gundersen, "Myonuclei acquired by overload exercise precede
  hypertrophy and are not lost on detraining", *PNAS* 107(34):15111-6, 2010-08-24 (mice; the extra
  nuclei were kept at least 3 months after the nerve was cut; later rodent work with ordinary
  training is mixed).
- Clarke, *Profiles of the Future*, revised edition, 1973 (the third law, paraphrased here; its
  exact wording should be checked against the book before it is quoted).

Read 2026-11-03 through search summaries of the papers and their abstracts, not the full texts; a
build that quotes a number from them should check the paper itself.

## Appendix C. Stale or conflicting items found while writing this

1. **Fourteen fantasy abilities are castable everywhere** (4.1), and data/abilities.csv's header says
   "Real mode shows real+tech" about a Real mode that was removed on 2026-07-16. progression-skills-gear.md
   pillar 4 has the same stale wording. Rung 2 closes both.
2. **Ability names that read as borrowed.** Several fantasy rows carry the spell names of well-known
   commercial games (for example Divine Shield, Holy Light, Consecrate, Frost Nova, Polymorph, Arcane
   Explosion, Shadow Bolt, Magic Missile), against the original-work rule. "Summon Undead" ("Raise the
   dead to serve you") conflicts with the setting's lore of death. Question 12.
3. **Cauterize** (pyromancy, fantasy) is a magic heal under a real medical word, which a teaching game
   should not blur (4.4).
4. **Worn gear's mass is not counted** toward the load (the inventory tick), against his carrying rule
   "mass still applies"; and **the backpack's 65 L exists whether or not a backpack is worn**
   (`default_volume_capacity`). Rung 3.
5. **Two experience curves.** data/skills/xp_curve.json (the web pages: a 10-level table) and the
   game's `xp_per_level` times level to the 1.5 with per-skill maximums of 15 to 30. The web's profile
   and the game can show different levels for the same experience.
6. **The death mode is per install** (`AppConfig::death_realistic`), while Hardcore is per character.
   A person with two characters cannot have one Realistic and one Simplified. Recommended with rung 5:
   the death mode moves into the character's save, and Settings shows the current character's.
7. **The setting design's 4.2** charged a Simplified death beyond the link the pack, against his
   Simplified rule (3.6); its 4.3 option 3 rejected any experience penalty, which his (b) reopens
   (section 3); its recorder lines are listed in 2.11.
8. **data/aging_fitness.ron** authors fitness levels and exercises that nothing reads (AgingSystem is
   deferred); its exercises train an `athletics` skill that data/skills/skills.csv does not have.
