# Sound and Hearing

Sound is something shaking, and the shake passing through the air, or
water, or a wall, until it shakes your eardrum. That is nearly the whole
of it. Every voice, engine, echo, thunderclap and ringing ear is that one
idea at work: a vibration, a material to carry it, and an ear at the end.

Knowing how it behaves is useful in ordinary ways. It tells you how far
away a thunderstorm is, and when to go indoors. It tells you why a
generator is so much quieter at the far end of the garden than beside the
back door, and why a gap under a door lets in so much noise. It lets you
hear a machine going wrong before it breaks.
And above all it tells you how loud is too loud, because the most common
way sound hurts people is slowly, painlessly and permanently, through
their hearing.

The physics in this guide comes from the Navy's electronics training
course, *Module 10, Introduction to Wave Propagation, Transmission Lines,
and Antennas* (1998), which teaches sound waves from the beginning. The
hearing advice comes from the National Institute on Deafness and Other
Communication Disorders (NIDCD, part of the National Institutes of
Health) and the National Institute for Occupational Safety and Health
(NIOSH); the decibel arithmetic and noise control from the Occupational
Safety and Health Administration (OSHA); and thunder from the National
Weather Service. All are United States government publications. Where
something is general practice, arithmetic or our own reasoning rather
than a published rule, the text says so.

Some neighbouring guides: [Light, Lenses and
Mirrors](/library#light-lenses-and-mirrors) covers the other kind of wave you
meet every day, [Keeping Things Working](/library#keeping-things-working) and
[Working Out Why Something Broke](/library#working-out-why-something-broke)
use your ears to find faults, and [Units and Converting
Them](/library#units-and-converting-them) has the feet, metres and seconds
the arithmetic below leans on.

## First: where sound hurts people

- **Noise that is too loud for too long.** The NIDCD says sounds at or
  below 70 A-weighted decibels (dBA) are unlikely to cause hearing loss
  even after long exposure, but "long or repeated exposure to sounds at
  or above 85 dBA can cause hearing loss." The louder the sound, the
  shorter the time it takes. The damage is to the hair cells of the
  inner ear, and the NIDCD is plain about them: "Unlike bird and
  amphibian hair cells, human hair cells don't grow back." Hearing lost
  this way does not come back.
- **One very loud bang.** Gunshots and explosions, the NIDCD says, can
  rupture the eardrum or damage the bones of the middle ear, and that
  kind of hearing loss "can be immediate and permanent." Its hearing
  protector fact sheet says the sound of a gunshot at close range can
  cause immediate and permanent damage. It puts a fireworks display at
  140 to 160 dBA.
- **The damage does not hurt.** NIDCD: because noise damage is usually
  gradual, "you might not notice it". Ringing in the ears (tinnitus) and
  sound that seems muffled after a noisy day are the warnings you do
  get.
- **Sudden loss of hearing is an emergency.** If hearing goes from one
  or both ears all at once, or over a few days, with or without a pop,
  ringing, dizziness or a full feeling in the ear, the NIDCD says "you
  should consider sudden deafness symptoms a medical emergency and visit
  a doctor immediately." It warns that people put it off thinking it is
  earwax, allergies or a sinus infection, and that delay can make
  treatment less effective.
- **Thunder means lightning can reach you.** The National Weather Service
  says thunder can be heard only about 10 miles from the strike, and "if
  you can hear thunder, chances are that you're within striking distance
  of the storm." Its rule: "When Thunder Roars, Go Indoors!" Then wait 30
  minutes after the last lightning or thunder before going back out.
- **Children cannot protect their own ears.** Among the NIDCD's
  prevention steps: "Protect the ears of children who are too young to
  protect their own." Its fact sheet says earmuffs are easier than
  earplugs to use correctly, especially for young children.
- **Headphones.** The NIDCD puts music through headphones at maximum
  volume at 94 to 110 dBA. Turning the volume down is one of the three
  best protections it names, with avoiding loud sound and moving away
  from it.

A rough test you can use anywhere comes from NIOSH's own toolbox talk on
noise (September 2026): if you have to shout to be heard by someone about
3 feet away, an arm's length, wear hearing protection.

## What sound is

### A source, something to carry it, and an ear

The Navy course names three things every sound needs: a source that
vibrates, a medium to carry the vibration (air, water, metal and so on),
and a detector to receive it. Take any one away and there is no sound.

Its experiment proves the middle one. An electric bell hangs inside a
glass jar, and the air is pumped out while the bell rings. The ringing
gets weaker and weaker; with a perfect vacuum, the course says, it would
be completely inaudible, though you could still see the clapper strike.
Let the air back in and the sound returns. Sound cannot cross empty
space. (In space, an explosion you can see makes no sound you can hear,
however close it is; our example, from the same rule.)

### Waves of squeeze and stretch

The vibration travels as what the course calls compression waves: the
air in front of the shaking object is pushed together, then pulled apart,
over and over, and that pattern of squeezes and stretches moves outwards
while the air itself only shuffles back and forth. A tuning fork, a
loudspeaker cone and your vocal cords all do the same thing.

### Frequency and pitch

The number of vibrations each second is the **frequency**, measured in
hertz (Hz): one hertz is one vibration a second. The course ties it to
what you hear: a high frequency is a high **pitch**, like a police
whistle, and a low frequency a low one, like the heavy strings of a
violin. It gives the normal range of human hearing as about 20 to
20,000 hertz, and notes that the range "varies with each individual."
OSHA's technical manual adds that as people begin to lose their hearing,
from age or from noise, they often lose the quiet high sounds first.

### Wavelength

The **wavelength** is the distance from one squeeze to the next. The
course's rule: divide the speed of the wave by its frequency.

**Worked example** (arithmetic). Taking sound in air as about 335 metres
a second (the next section):

| Frequency | Wavelength in air |
|---|---|
| 20 Hz, the bottom of normal hearing | about 17 m |
| 100 Hz, a low hum | about 3.4 m |
| 1,000 Hz | about 34 cm |
| 10,000 Hz, a high hiss | about 3.4 cm |
| 20,000 Hz, the top of normal hearing | about 1.7 cm |

That spread explains things you notice at home. Long, low waves bend
round corners and pass through walls far better than short, high ones,
which is why the music next door arrives as a thudding bass with the
words missing (general physics, our example).

### Loudness is not the same as energy

The course separates a sound's **intensity**, the energy it carries, from
its **loudness**, the sensation in your ear. They rise together but not
in step: doubling the loudness of a sound takes about a tenfold increase
in its intensity. That is why sound is measured on a scale that counts in
multiples of ten, the decibel, below.

### Quality

Two instruments playing the same note at the same loudness still sound
different. The course calls this the **quality** of a sound: most sounds
are not one pure frequency but a mixture, and the mixture is what lets
you tell a violin from a flute, or one person's voice from another's.
Quality is also what tells you a machine has changed its tune.

## How fast sound travels

The Navy course gives the speed of sound in air at freezing point (32
degrees F, 0 C) as 1,087 feet a second, and says that for practical
purposes it may be taken as 1,100 feet a second. That is about 335
metres a second (arithmetic). Warmer air carries it slightly faster; the
course says raising the temperature of the medium increases the speed.
How loud or how high the sound is makes no difference to its speed.

It travels faster still in water and solids. The course's table puts
sound in fresh and salt water at roughly 4,600 to 4,950 feet a second,
more than four times its speed in air, and in steel at about 16,400 to
16,850 feet a second; it says the speed in solids such as steel and glass
is about 15 times the speed in air. Sound is fastest in hard materials,
slower in liquids, and slowest in gases.

### Worked example: how far away is the storm?

Light reaches you, for practical purposes, at once. Sound does not. The
National Weather Service says thunder takes about 5 seconds to travel a
mile: "If you count the number of seconds between the flash of lightning
and the sound of thunder, and then divide by 5, you'll get the distance
in miles to the lightning". Its examples: 5 seconds is 1 mile, 15
seconds is 3 miles, and 0 seconds is very close. In metric, about 3
seconds is 1 kilometre (arithmetic: 1,000 metres at 335 metres a second
is 3 seconds).

Two things the weather service adds, and they matter more than the
arithmetic:

- **Count from somewhere safe.** "Keep in mind that you should be in a
  safe place while counting." The count is not a way to decide whether
  you can stay out. If you can hear thunder at all, you are probably
  within reach. Substantial buildings and hard-topped vehicles are safe;
  rain shelters, small sheds and open vehicles are not.
- **The sound tells you about the path.** A sharp crack or click means
  the lightning passed nearby; a rumble means it was at least several
  miles away. The weather service explains that a lightning channel runs
  many miles through the air, so you hear the part nearest you first and
  then parts farther and farther away. The Navy course adds the echoes:
  by the time the sharp sound of lightning reaches a distant listener,
  reverberation has usually drawn it out into the roll we call thunder.

### Worked example: an echo as a tape measure

An echo is sound that has gone out, bounced and come back, so it has
travelled the distance twice (arithmetic). Stand facing a big flat wall,
a barn or a cliff, clap once, and time the echo.

- An echo half a second after the clap: the sound went 335 x 0.5 = 168
  metres in all, so the wall is about 84 metres away.
- An echo too quick to separate from the clap means the wall is close;
  the time is too short to count by ear (general practice).

The Navy uses exactly this idea at sea. Its depth finder, the course
explains, sends a pulse of sound down from the ship and times the echo
from the sea floor.

### Wind bends sound

The course explains why a sound carries well one day and not the next.
Wind blows faster a little above the ground than right at it, because
the ground slows the lowest air. Sound moving with the wind is carried
faster at the top of the wave than at the bottom, so the wave tips over
and follows the ground. The result, in the course's words: refraction
"causes sound to travel farther with the wind than against it." A
neighbour's generator you cannot hear on most days may be plain on the
day the wind blows from their side (our example).

## Echoes, rooms, resonance and moving sources

- **Echo.** The course: a reflected sound is never as loud as the
  original, because some of the energy is absorbed by the surface it
  bounces from.
- **Reverberation.** In an empty room or other enclosed space, sound
  reflects many times over, which the course calls reverberation. It
  makes a sound seem to last longer. An empty house sounds hollow and
  loud for this reason, and the same house with carpets, curtains and
  furniture sounds calmer (our example).
- **Resonance.** A cavity has a natural frequency of its own. The course's
  example is a person making noises into an empty barrel: at one pitch
  the tone suddenly sounds much louder, because the voice matches the
  barrel's own frequency and the barrel joins in. A panel that buzzes
  only at one engine speed is doing the same thing (our example).
- **The Doppler effect.** When a sound source and a listener move
  towards each other, the pitch heard rises; when they move apart, it
  falls. The course's example is a train whistle, higher as the train
  approaches than after it passes, though the whistle itself never
  changes.

## Decibels: the scale of loudness

Sound covers an enormous range. OSHA's technical manual says the
greatest sound pressure that can be perceived without pain is about 10
million times the faintest that can be heard. A scale that counts in
ordinary steps would be unusable, so sound is measured in **decibels**
(dB), which count in multiples. NIOSH's explanation: decibels are
logarithmic, and a sound 10 dB louder than another is ten times more
intense. Put that beside the Navy course's tenfold-intensity rule above
and you get a rough guide: 10 dB more sounds about twice as loud (our
arithmetic, from the two sources).

**dBA** is the decibel reading weighted to match the human ear. OSHA's
manual says the A-weighting responds to sound much like your ear does,
most of all to the middle frequencies, 500 to 4,000 hertz, and that it is
thought to predict the damage a noise will do to hearing. Every hearing
limit in this guide is in dBA.

Three rules from OSHA's technical manual do most of the everyday work:

1. **Decibels do not add like ordinary numbers.** Decibels are
   logarithmic, "so it is not correct to sum multiple sound values using
   arithmetic addition." Two mowers at 90 dBA each do not make 180.
2. **Doubling the source adds 3 dB.** "As a general rule, doubling the
   sound power increases the noise level by 3 dB." Two equal mowers make
   about 93 dBA; four make about 96 (arithmetic, from the rule).
3. **Doubling the distance takes off 6 dB, outdoors.** In open air with
   nothing to reflect it, the level from a small source falls by 6 dB
   each time the distance doubles. OSHA's example: 90 dB at 1 metre, 84
   dB at 2 metres, 78 dB at 4 metres. Indoors, reflections from walls
   keep it from falling that fast: less than 6 dB per doubling.

### Familiar sounds

The NIDCD's averages:

| Sound | Level |
|---|---|
| Normal conversation | 60 to 70 dBA |
| Movie theatre | 74 to 104 dBA |
| Lawnmowers | 80 to 100 dBA |
| Motorcycles and dirt bikes | 80 to 110 dBA |
| Music through headphones at maximum volume, sporting events and concerts | 94 to 110 dBA |
| Sirens | 110 to 129 dBA |
| Fireworks display | 140 to 160 dBA |

(The lawnmower row is from the NIDCD's hearing protector fact sheet and
the movie theatre row from its web page on noise; the rest are on both.)

### Worked example: moving the generator

A generator measures 85 dBA at 2 metres (our example figure). Outdoors,
with nothing to reflect the sound, OSHA's 6 dB rule gives about 79 dBA
at 4 metres, 73 at 8 metres and 67 at 16 metres (arithmetic). Every
doubling of distance cuts it by the same amount, so the first few metres
matter most. The same arithmetic says that standing right beside it, at
half a metre, you would be near 97 dBA.

There is a second reason to put a generator far away, and it is the
bigger one: its exhaust. The Consumer Product Safety Commission's rules
for running engines outdoors and away from the house are in [Keeping
Things Working](/library#keeping-things-working).

## How long is too long

### NIOSH's limit

NIOSH's criteria document for noise, *Occupational Noise Exposure*
(revised criteria, June 1998), sets its recommended exposure limit at 85
dBA averaged over an 8-hour day, with what it calls a 3-decibel exchange
rate. NIOSH's page on noise exposure explains it: "For each 3 dBA
increase in noise level, NIOSH recommends reducing the exposure duration
by half." The criteria document's Table 1-1 gives the combinations no
worker exposure should equal or exceed:

| Level | Time a day |
|---|---|
| 85 dBA | 8 hours |
| 88 dBA | 4 hours |
| 91 dBA | 2 hours |
| 94 dBA | 1 hour |
| 97 dBA | 30 minutes |
| 100 dBA | 15 minutes |
| 110 dBA | about 1 and a half minutes |

It also sets a ceiling: exposure to continuous, varying, intermittent or
impulsive noise "shall not exceed 140 dBA."

Two things follow that surprise most people. A level that sounds only a
little louder halves your safe time. And a day is one budget: an hour
with the mower and an hour with the leaf blower add together.

### OSHA's limit, and which to use at home

OSHA's legal limit for workplaces, 29 CFR 1910.95, is looser: 90 dBA for
8 hours, halving the time for every 5 dB (Table G-16 runs 90 for 8 hours,
95 for 4, 100 for 2, 105 for 1, 110 for half an hour and 115 for a
quarter of an hour or less), with a hearing conservation programme
required from an 8-hour average of 85 dBA. It says impulsive or impact
noise "should not exceed 140 dB peak sound pressure level." OSHA's own
technical manual notes that NIOSH's 1998 document recommends the 3 dB
rate rather than 5.

At home there is no employer and no programme, and your ears do not know
whose rule applies. Using NIOSH's stricter table is our recommendation.

### Worked example: a Saturday of yard work

The NIDCD puts lawnmowers at 80 to 100 dBA. Say your mower, measured at
your ear, reads 94 dBA (our example figure). NIOSH's table gives you 1
hour unprotected. Mow for two hours without protection and you have spent
the whole day's allowance twice over, before the trimmer or the chainsaw
starts (arithmetic). With earplugs that take 9 dB off at the ear (the
next section's worked example), the mower is about 85 dBA at the ear,
and the table gives 8 hours.

### Measuring it

The NIDCD's hearing protector fact sheet points to NIOSH's free Sound
Level Meter app for iOS devices as one example of a decibel meter app
for checking the sounds around you. Hold the phone where your ear is,
not against the machine: OSHA's manual warns that readings taken very
close to a noise source are not reliable, because small changes in
position give big differences. A phone is a guide, not a calibrated
instrument (general practice).

### The warning you get

The NIDCD says a loud exposure sometimes causes a temporary hearing loss
that disappears 16 to 48 hours later, and adds that research suggests
there may be long-term damage even though the hearing seems to come
back. Muffled hearing or ringing after a noisy day is not a sign that
you got away with it; it is the sign that the day was too loud.

## Protecting your hearing

### First, less noise

The NIDCD's fact sheet puts it in order: the best protection is to avoid
loud sound, move away from the noise, or turn the volume down, and
hearing protectors are for when those are not possible. OSHA's manual
calls the same order a hierarchy, treating the source first, then the
path, then the person, with hearing protection "the last line of
defense". At home that means: choose the quieter machine, keep it
maintained (a loose guard or a worn bearing is louder), put distance
between you and it, and only then reach for earplugs (our household
reading).

### When to wear them

The NIDCD's fact sheet recommends hearing protectors at auto races,
sporting events, fireworks displays and concerts; when riding a
motorcycle, dirt bike or snowmobile, or driving an all-terrain vehicle
or a tractor; at band or orchestra rehearsals and performances; in
industrial, warehouse, farm, landscape and other loud workplaces; and for
shooting sports. Make it a habit, it says, and keep earplugs or earmuffs
handy for unexpected loud noises. For gunfire, which can do its damage in
one shot, earplugs and earmuffs worn together are our recommendation,
following NIOSH's advice to double up for the loudest exposures.

### Foam earplugs

The NIDCD's fact sheet gives these steps for formable foam earplugs:

1. Gently roll the earplug between your fingers into a thin tube,
   without creasing the foam, because creases make tunnels for sound.
2. With the opposite hand, pull the top of your ear up and back to
   straighten the ear canal.
3. Keep rolling the plug and gently slide it into the canal. It should
   fit evenly across the opening.
4. Hold it in with your finger for 20 to 30 seconds while it expands.
5. Check the fit: comfortable, and barely visible when it is in right.
6. Repeat on the other ear.
7. Do not cut or tear foam earplugs to make them fit; it makes them work
   less well. If foam plugs cannot be fitted, use another kind of
   protector.
8. To take them out, twist slowly to break the seal and then ease them
   out.

It adds: make sure your hands and the earplugs are clean first; your own
voice should sound different, louder or muffled, once they are in; and
"Never force earplugs into your ears." If you cannot get a comfortable
fit, use earmuffs instead. NIOSH's toolbox talk adds washing your hands
before putting earplugs in, to help prevent infection. The fact sheet
says foam plugs are meant for one use but can be reused if they are
clean and still expand back to their original shape.

### Earmuffs

Earmuffs cover the whole ear. The NIDCD says they are easier to use
correctly than earplugs, especially for young children, and come in
sizes for children and adults, but that the arms of glasses, hairstyles,
hats and facial hair can make gaps. OSHA's manual is blunt about gaps:
"even a very small leak in the seal can destroy the effectiveness of the
earmuff." If you wear glasses, check the seal.

### What the number on the box means

The **noise reduction rating (NRR)** printed on a hearing protector's
packaging is a laboratory figure the EPA requires on every protector sold
in the United States, NIOSH's criteria document explains. NIOSH says
people in real use get less than the label: it recommends subtracting 25
percent of the NRR for earmuffs, 50 percent for formable earplugs and 70
percent for all other earplugs. Then, to compare with a dBA reading,
subtract 7 dB more, which is also the first step in OSHA's own method
(29 CFR 1910.95, Appendix B).

**Worked example** (arithmetic, using NIOSH's method):

- Foam earplugs labelled NRR 32: half off for formable plugs leaves 16;
  minus 7 leaves about 9 dB. A 100 dBA machine is still about 91 dBA at
  your ear, which NIOSH's table allows for 2 hours.
- Earmuffs labelled NRR 25: a quarter off leaves about 19; minus 7
  leaves about 12 dB.

The label number is a laboratory figure, and in ordinary use you get
less, which is the whole reason for NIOSH's derating. NIOSH says that
where exposures exceed 100 dBA as an 8-hour average, people should wear
earplugs and earmuffs together.

### Children

Protect their ears for them. Earmuffs sized for children are the
NIDCD's easier option. Fireworks, at 140 to 160 dBA on the NIDCD's
table, reach and pass the 140 dBA ceiling NIOSH sets for any exposure at
all; watching from farther away is the protection that costs nothing
(our reading of the NIDCD's distance advice). The NIDCD's fact sheet also notes that if a
loud noise happens suddenly, you can cover your ears with your hands and
move away.

### Get your hearing tested

The NIDCD's last prevention step is to have your hearing tested if you
think you might have hearing loss. NIOSH's toolbox talk goes further for
anyone who works around noise: have a test early to get a baseline, and
recheck it regularly to catch changes.

## Keeping sound out of a room

OSHA's technical manual, written for factories, carries rules that work
just as well for a bedroom next to a pump house.

- **Seal the gaps first.** On noise barriers: "Any gap through which air
  can pass will allow a significant amount of noise to pass as well." A
  door with a gap under it, an unsealed pipe hole or a vent leaks more
  sound than the solid wall around it (our household reading). Weather
  stripping and a door sweep are often the cheapest quiet you can buy
  (general practice).
- **Soaking up echo is not the same as blocking sound.** The manual says
  adding sound-absorbing material to a room cuts the reflected sound
  inside it but does nothing to the sound that travels straight from the
  source. Foam tiles, curtains and rugs make a room less echoing; they do
  little to stop the neighbour's music coming through the wall (our
  example).
- **Treat the source.** A vibrating metal panel can radiate noise; the
  manual describes damping material fixed to panels as an effective cure
  for that kind of ringing. Standing a washing machine or pump on a
  rubber mat or proper mounts keeps its shaking out of the floor (general
  practice).
- **Use distance.** The 6 dB per doubling rule works outdoors in your
  favour. Put the noisy thing far away, and put a building, not a fence
  with gaps, between it and where you sleep (our application of the
  rule).

## Listening as a tool

Your ears are the first instrument for checking a machine, as [Keeping
Things Working](/library#keeping-things-working) explains: learn what each
machine sounds like when it is running well, so that a new squeal,
knock, grind or hum stands out. A change of pitch at a steady speed is
worth noticing, because pitch is frequency, and a new frequency means
something new is vibrating (our application of the physics).

Mechanics use a stethoscope made for the purpose to listen to bearings
and engines, because sound travels well through solid metal. If you do
this, it is general practice to keep hair, sleeves, cords and the tool
well clear of anything that moves or is hot, and never to touch anything
electrical. When a machine has stopped working, [Working Out Why
Something Broke](/library#working-out-why-something-broke) is the next step.

## Know where your own work stops

- **Sudden hearing loss** in one or both ears: a doctor immediately
  (NIDCD).
- **Ringing that does not fade, or hearing that stays muffled,** after a
  loud exposure: have your hearing tested (NIDCD).
- **Help choosing hearing protection, or custom-made earplugs:** the
  NIDCD's fact sheet says to consult a hearing health professional.
- **An ear that hurts, bleeds, leaks fluid, or was hit by a blast:** a
  health professional, not a home remedy (general practice).
- **Lightning:** no amount of counting makes outdoors safe. Go indoors
  (National Weather Service).

## How the game models it

The game plays sound, but it does not yet simulate how sound moves, and
it does not model hearing at all.

- **Volume.** Settings > Audio has Master, Music and SFX (sound effects)
  volume sliders, and a separate control for interface clicks. A video
  playing on a screen in the world is a world sound on the SFX slider.
- **Screens fade with distance, in a straight line.** Every screen in
  the world starts muted until you press Unmute. Once its sound is on,
  it is loudest at the screen and fades in a straight line to silence at
  50 metres, and it leans towards the ear on the side the screen is on,
  so turning round swaps it between your ears. Real sound does not stop
  at a fixed distance: outdoors it loses about 6 dB each time the
  distance doubles, as OSHA describes, so it fades fast at first and
  then slowly, and never quite to nothing.
- **Footsteps and doors.** Walking plays a footstep every 1.5 metres,
  grass on a planet's ground and metal anywhere else. The doors of your
  home aboard the station sound when they open and close if you are
  within 25 metres, and a door you built sounds when you open or shut
  it. None of these is placed in space: a door 20 metres away sounds as
  loud as one beside you.
- **No travel time, no echoes, no walls.** Every sound plays at the
  moment its cause happens, so there is no gap between a flash and its
  sound. A screen's sound reaches you through walls as if they were not
  there, and no room echoes. Thunderstorms in the game's weather make no
  thunder at all, so you cannot practise the flash-to-thunder count
  there.
- **No hearing damage.** Nothing in the game harms your hearing, and
  there are no earplugs or earmuffs to wear. Some machine records carry a
  noise figure (the diesel generator in `data/electrical.ron` is listed
  at 95 dB), but nothing in the game reads it yet.

What the game leaves out, so you do not learn it from the game: the
speed of sound, echoes, the way walls block and gaps leak, the loudness
that damages hearing, and the time it takes. In the game you can stand
beside a roaring generator all day with no cost. Do not carry that habit
outside.

## You own this when

- You know that hearing lost to noise does not come back, and that the
  damage does not hurt while it happens.
- You can use the raised-voice test, and you wear protection when you
  have to shout at arm's length.
- You know NIOSH's 85 dBA for 8 hours, and that every 3 dB more halves
  the time.
- You can fit a foam earplug properly, check an earmuff's seal, and
  explain why the NRR on the box is more than you will get.
- You protect children's ears for them.
- You treat sudden hearing loss as an emergency.
- You go indoors when you hear thunder, and wait 30 minutes after the
  last of it.
- You can count from lightning to thunder and turn it into a distance,
  and use an echo to measure a long way off.
- You know that decibels do not add like ordinary numbers, that two equal
  sources add 3 dB, and that doubling your distance outdoors takes off 6.
- You can explain why bass comes through a wall when words do not, and
  why a gap under a door matters more than the door.
- You listen to your machines, and you notice when one changes its tune.

## Sources

Grouped by what kind of authority each one is. Every source here is a
work of the United States federal government, and its own text is in the
public domain. Regulations were read in the Electronic Code of Federal
Regulations on 3 October 2026; the eCFR is updated in place. Web pages
were read on 3 October 2026.

### United States government (public domain)

- US Navy. *Navy Electricity and Electronics Training Series, Module 10,
  Introduction to Wave Propagation, Transmission Lines, and Antennas*,
  NAVEDTRA 14182, September 1998, Distribution Statement A, approved for
  public release. Chapter 1 (the three requirements for sound: source,
  medium and detector; the bell in a jar from which the air is pumped
  out; compression waves; frequency and pitch, with the police whistle
  and violin strings; the normal hearing range of about 20 to 20,000
  hertz, varying with each individual; wavelength as velocity divided by
  frequency; intensity and loudness, and the tenfold increase in
  intensity to double the loudness; quality; the speed of sound in air,
  1,087 feet a second at 32 degrees F and 1,100 for practical purposes;
  warmer media carrying sound faster; the table of speeds in water and
  steel, and solids such as steel and glass carrying it about 15 times
  faster than air; echo, with the fathometer; refraction by wind, and
  sound carrying farther with the wind than against it; reverberation,
  and lightning's sharp sound drawn out into thunder by it; resonance and
  the barrel; the Doppler effect and the train whistle).
  Copy hosted in the Internet Archive's NEETSModules collection.
  https://archive.org/details/NEETSModules
- National Institute on Deafness and Other Communication Disorders
  (National Institutes of Health). Noise-Induced Hearing Loss, NIH
  Publication No. 14-4233, text updated March 2014, page last updated 16
  April 2025 (70 dBA unlikely to cause hearing loss and 85 dBA and above
  able to; the table of familiar sounds; hair cells that do not grow
  back; impulse sounds that can be immediate and permanent; tinnitus;
  temporary loss that disappears 16 to 48 hours later, possibly with
  lasting damage; the prevention steps, including protecting children's
  ears and having your hearing tested).
  https://www.nidcd.nih.gov/health/noise-induced-hearing-loss
- National Institute on Deafness and Other Communication Disorders.
  *Hearing Protectors*, NIDCD fact sheet, NIH Pub. No. 20-DC-8122,
  November 2020 (avoid, move away or turn down first; the gunshot at
  close range; lawnmowers 80 to 100 dBA; NIOSH's Sound Level Meter app;
  the settings and activities where protectors are recommended, shooting
  sports among them, and keeping them handy;
  the NRR on most protectors; the steps for formable foam earplugs, clean
  hands, not cutting the foam and twisting to remove; earmuffs easier
  for young children and the gaps from glasses, hair, hats and facial
  hair; covering your ears with your hands for a sudden loud noise;
  consulting a hearing health professional).
  https://www.nidcd.nih.gov/sites/default/files/documents/health/hearingprotectors.pdf
- National Institute on Deafness and Other Communication Disorders.
  Sudden Deafness, page last updated 14 September 2018 (sudden
  sensorineural hearing loss, its signs, and "you should consider sudden
  deafness symptoms a medical emergency and visit a doctor
  immediately"; delay can decrease the effectiveness of treatment).
  https://www.nidcd.nih.gov/health/sudden-deafness
- National Institute for Occupational Safety and Health. *Criteria for a
  Recommended Standard: Occupational Noise Exposure, Revised Criteria
  1998*, DHHS (NIOSH) Publication No. 98-126, June 1998 (the REL of 85
  dBA as an 8-hour time-weighted average with a 3-dB exchange rate;
  Table 1-1 of levels and durations; the 140 dBA ceiling; the NRR that
  the EPA requires on every hearing protector's label; derating the NRR
  by 25, 50 and 70 percent for earmuffs, formable earplugs and other
  earplugs, and subtracting 7 dB more when comparing with A-weighted
  levels; earplugs and earmuffs together above 100 dBA).
  https://stacks.cdc.gov/view/cdc/6376
- National Institute for Occupational Safety and Health. Understand
  Noise Exposure, page dated 31 January 2024 (the REL of 85 dBA over an
  eight-hour shift; "For each 3 dBA increase in noise level, NIOSH
  recommends reducing the exposure duration by half"). And Occupational
  Hearing Loss, page dated 13 April 2026 (decibels are logarithmic; a
  sound 10 dB louder is ten times more intense). Both read through a page
  summary, because cdc.gov refuses scripted downloads.
  https://www.cdc.gov/niosh/noise/prevent/understand.html
  https://www.cdc.gov/niosh/noise/about/index.html
- National Institute for Occupational Safety and Health, with CPWR.
  *Noise and Hearing Protection*, toolbox talk, DHHS (NIOSH) Publication
  No. 2026-118, September 2026 (the raised-voice test at 3 feet; wash
  your hands before inserting earplugs; a baseline hearing test early,
  rechecked regularly). Restated here rather than quoted, because it was
  produced with a private partner.
  https://www.cdc.gov/niosh/media/pdfs/2026/09/2026-118.pdf
- Occupational Safety and Health Administration. *OSHA Technical
  Manual*, Section III, Chapter 5, Noise, updated 6 July 2022 (the
  threshold of pain about 10 million times the threshold of hearing;
  people losing their hearing often losing quiet high sounds first;
  decibels not summed by arithmetic addition; 6 dB less for each doubling
  of distance in a free field, with the 90, 84 and 78 dB example, and
  less than 6 dB indoors; doubling the sound power adds 3 dB; A-weighting
  and the 500 to 4,000 hertz range; readings in the near field
  unreliable; source, path and receiver, with hearing protection the
  last line of defense; any gap through which air passes letting noise
  through; sound absorption doing nothing for the direct sound; damping
  vibrating panels; a very small leak destroying an earmuff's
  effectiveness; NIOSH's 1998 recommendation of a 3 dB exchange rate).
  https://www.osha.gov/otm/section-3-health-hazards/chapter-5
- Occupational Safety and Health Administration. 29 CFR 1910.95,
  Occupational noise exposure (Table G-16, 90 dBA for 8 hours down to
  115 dBA for a quarter of an hour; impulsive or impact noise "should not
  exceed 140 dB peak sound pressure level"; the hearing conservation
  programme from an 8-hour average of 85 dBA, the action level; Appendix
  B, subtracting 7 dB from the NRR for A-weighted levels).
  https://www.ecfr.gov/current/title-29/subtitle-B/chapter-XVII/part-1910/subpart-G/section-1910.95
- National Weather Service. Understanding Lightning: Thunder, undated
  (thunder heard only about 10 miles from the strike; about 5 seconds to
  travel a mile, divide by 5 for miles; be in a safe place while
  counting; a crack or click means nearby, a rumble at least several
  miles away; a channel many miles long, heard nearest part first). And
  Lightning Safety Tips and Resources and its overview,
  undated ("When Thunder Roars, Go Indoors!"; substantial buildings and
  hard-topped vehicles are safe, rain shelters, small sheds and open
  vehicles are not; wait 30 minutes after the last lightning or
  thunder).
  https://www.weather.gov/safety/lightning-science-thunder
  https://www.weather.gov/safety/lightning-safety-overview

### Inside this project

- Volume settings: `draw_audio_content` in `src/gui/pages/settings.rs`.
- Screen sound: `audio_placement`, `AUDIO_MAX_DISTANCE_M` (50 metres)
  and the muted start in `src/engine/screens/video.rs`.
- Footsteps (every 1.5 metres, grass on a planet and metal elsewhere)
  and the one-shot sound queue: the audio block of `src/lib.rs`; door
  sounds within 25 metres: `src/engine/home_meshes.rs`; the sound
  catalogue, `data/sounds.toml`.
- Machine noise figures that nothing reads yet: `noise_level_db` in
  `data/electrical.ron`.
- [Light, Lenses and Mirrors](/library#light-lenses-and-mirrors), [Keeping
  Things Working](/library#keeping-things-working), [Working Out Why Something
  Broke](/library#working-out-why-something-broke) and [Units and Converting
  Them](/library#units-and-converting-them).

### Labelled in the text as general practice, arithmetic or our reading, not sourced

- That an explosion in space makes no sound, as an example of the Navy
  course's vacuum rule.
- The wavelength table, the metric speed of sound (about 335 metres a
  second) and the 3 seconds per kilometre rule: arithmetic from the
  Navy course and the weather service.
- Bass coming through walls when words do not: general physics.
- The echo arithmetic, and that a very quick echo is too short to time
  by ear.
- The neighbour's generator on a downwind day; the empty house that
  sounds hollow; the panel that buzzes at one engine speed.
- That 10 dB more sounds about twice as loud: arithmetic combining NIOSH's
  tenfold intensity with the Navy course's tenfold-for-double rule.
- The two-mower and four-mower figures, the generator distances and the
  yard-work exposure: arithmetic from OSHA's and NIOSH's rules, with our
  own example figures.
- Using NIOSH's stricter limit at home rather than OSHA's.
- Holding a phone meter at your ear, and that a phone is a guide rather
  than a calibrated instrument.
- Quieter machines, maintenance, distance before earplugs: our household
  reading of the hierarchy of controls.
- The NRR worked examples: arithmetic using NIOSH's method.
- Earplugs and earmuffs together for gunfire, extending NIOSH's advice
  to double up above 100 dBA.
- Watching fireworks from farther away, as a reading of the NIDCD's
  distance advice.
- Weather stripping and door sweeps; rubber mats and mounts under
  machines; putting a building between the noise and where you sleep.
- That a change of pitch means something new is vibrating; keeping
  clear of moving parts when listening with a mechanic's stethoscope.
- Seeing a health professional for an ear that hurts, bleeds, leaks or
  was hit by a blast.
