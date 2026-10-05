# Electricity and How It Flows

Electricity is a flow of electric charge around a closed loop. A voltage
pushes it, the wire carries it, and whatever it passes through holds it
back. Three numbers describe nearly everything that happens: the voltage
(the push), the current (how much flows) and the resistance (how hard it
is to get through). A fourth, the power, says how fast the flow is doing
work: lighting a lamp, turning a motor, or heating a wire. Once you can
move between those four, you can see why a thin extension cord gets warm,
why a loose connection starts fires, why a circuit breaker does not
protect a person, and why a current far too small to light a bulb can
stop a heart.

This guide is the physics. Its neighbours are the practice: [Where Your
Own Electrical Work Stops](where_your_electrical_work_stops.md) is about
which jobs on a house's wiring are yours and which are not, [Working Out
Why Something Broke](working_out_why_something_broke.md) holds the
procedure for proving a circuit dead before you touch it, [Power
Tools](power_tools.md) covers the electric tools in your hands, and [Your
First Solar Power](first_solar_power.md) and [Batteries and
Storage](batteries_and_storage.md) cover making and storing it.

The physics comes from the US Navy's electricity course for beginners,
the *Navy Electricity and Electronics Training Series* (Modules 1 and 2,
1998), and from the National Bureau of Standards' copper wire tables
(1966). The safety material comes from the Occupational Safety and Health
Administration (OSHA), the National Institute for Occupational Safety and
Health (NIOSH), the Consumer Product Safety Commission (CPSC) and the
National Weather Service. All are United States government publications,
in the public domain. Where something is arithmetic, general physics or
our own explanation rather than a published statement, the text says so.

## First: where electricity hurts people

- **Current through the body.** The Navy's course puts it in one line:
  "Fundamentally, current, rather than voltage, is the measure of shock
  intensity." How much current it takes, and why the path and the time
  matter as much as the amount, is the heart of this guide (below).
- **Burns.** OSHA: "Burns are the most common shock-related injury." It
  describes three kinds: electrical burns, where current flowing through
  tissue or bone heats it from inside; arc or flash burns from the heat
  of an electric arc nearby; and thermal burns from touching overheated
  wires or equipment, or from clothing set alight. First aid, and why a
  small electrical burn is never a small injury, is in [Treating
  Burns](treating_burns.md).
- **Arcs and blasts.** When current jumps a gap through the air it makes
  an arc. NIOSH's manual for electrical trades students reports
  temperatures as high as 35,000 F in arc blasts, a pressure wave strong
  enough to throw a person, and droplets of molten copper and aluminium
  that can set ordinary clothing alight 10 feet or more away. NIOSH says
  arcs like that are often caused by equipment failure, and they are one
  reason the inside of a panel is an electrician's work (our reading; see
  [Where Your Own Electrical Work Stops](where_your_electrical_work_stops.md)).
- **Falls.** OSHA's booklet on hand and power tools warns that a shock
  "can cause the user to fall off a ladder or other elevated work surface
  and be injured due to the fall," and NIOSH adds that even a mild shock
  can make you lose your balance.
- **Fire.** NIOSH calls overloads "a major cause of fires": a wire made to
  carry more current than it can handle gets hot, sometimes inside a wall.
  Why, and why a loose connection does the same thing, is below.
- **Water.** OSHA explains that pure water is a poor conductor, but the
  small amounts of dissolved salts and other impurities in real water
  make it, and anything soaked in it, conduct. Wet skin is one of those
  things.
- **Overhead lines and lightning.** NIOSH: "Most people do not realize
  that overhead powerlines are usually not insulated." The rules for
  keeping yourself, a ladder or a pole 10 feet away from them are in
  [Roofs and Keeping Water Out](roofs_and_keeping_water_out.md).
  Lightning safety is in [Reading the Weather](reading_the_weather.md).

**If someone is being shocked, do not touch them.** NIOSH's first aid
sheet says to shut off the current, and if you cannot get to the switch
quickly, to pry the person away from the circuit with something that
does not conduct electricity, such as dry wood: "Do not touch the victim
yourself if he or she is still in contact with an electrical circuit!"
OSHA's booklet on electrical hazards adds that a severe shock "can cause considerably more
damage than meets the eye," including internal bleeding and damage to
tissues, nerves and muscles that cannot be seen, and says to seek
emergency medical help immediately after a shock. So: unplug it or switch
off the breaker, call 911, start CPR if the person is not breathing and
you are trained, and have every shock checked by a doctor even if the
person seems fine. The rest of the first aid is in [Treating
Burns](treating_burns.md).

## What is flowing

**Charge and current.** Electric charge is carried by electrons, and an
electric current is, in the Navy course's words, "a directed movement of
electrons in a conductor or circuit." Current is measured in amperes
(amps, A). One ampere is one coulomb of charge passing a point every
second. The National Institute of Standards and Technology (NIST) gives
the charge of one electron as exactly 1.602 176 634 x 10^-19 coulomb, so
one ampere is about 6.24 billion billion electrons going past every second
(our arithmetic). Small currents are counted in milliamperes (mA): one
thousandth of an ampere.

**Voltage.** A voltage is a difference between two points: how hard the
charge at one point is pushed towards the other. The Navy course explains
that in most circuits only the difference between two points matters, so
a voltage is always stated between two points, often a wire and the
ground (our wording). Its comparison is water: the flow through a pipe
joining two tanks depends on the difference in the water levels, and the
flow of current through a circuit depends on the difference in voltage
across it. Voltage is measured in volts (V).

**Resistance.** Some materials let current through easily and some
hardly at all. OSHA's booklet on electrical hazards calls the easy ones
conductors: metals, and also, easily forgotten, "the surface or
subsurface of the earth." The hard ones are insulators: glass, plastic,
porcelain, clay, pottery and dry wood. And: "Even air, normally an
insulator, can become a conductor, as occurs during an arc or lightning
stroke." Resistance is measured in ohms. The Navy course defines it by
the other two units: a conductor has one ohm of resistance "when an
applied potential of one volt produces a current of one ampere."

**Water and skin.** OSHA again: "Pure water is a poor conductor." But
impurities in water, such as salt or acid, turn water, and anything
soaked in it, into a conductor; dry wood conducts once it is saturated.
"The same is true of human skin. Dry skin has a fairly high resistance to
electric current. But when skin is moist or wet, it acts as a conductor."
Sweat counts: NIOSH lists wet clothing, high humidity and perspiration
among the things that raise the risk of electrocution.

**A circuit is a loop.** Current flows only around a complete path, out
from the source and back to it. OSHA lists the ways a person becomes part
of that path: touching both wires of a circuit; touching one live wire
and the ground; touching a metal part that has become live because its
insulation failed; or touching anything else that is carrying current.
The ground counts because, in house wiring, one of the two wires is
deliberately connected to the earth (below), so the earth, a water pipe
or a damp floor can carry current back towards the source whether you
meant it to or not (our explanation).

## Ohm's law

The Navy course states the relationship that ties the three numbers
together: "The current in a circuit is DIRECTLY proportional to the
applied voltage and INVERSELY proportional to the circuit resistance." As
a formula:

- current (amps) = voltage (volts) / resistance (ohms)
- so voltage = current x resistance, and resistance = voltage / current.

Double the voltage across the same resistance and the current doubles.
Double the resistance at the same voltage and the current halves. That is
all of it, and almost everything else in this guide is that formula
applied to something.

### Worked example: the same 120 volts, two different hands

NIOSH gives two figures for the resistance of a person's skin: "Dry skin
may have a resistance of 100,000 ohms or more. Wet skin may have a
resistance of only 1,000 ohms." Put each across the 120 volts of a
household outlet:

| Skin | Resistance | Current at 120 volts | In the table below |
| --- | --- | --- | --- |
| Dry | 100,000 ohms | 0.0012 A (1.2 mA) | about a faint tingle |
| Wet | 1,000 ohms | 0.12 A (120 mA) | breathing stops; death possible |

The arithmetic is ours; the resistances are NIOSH's. The same outlet,
the same voltage, and a hundred times the current, because of water.

Do not read the first row as "dry skin is safe." The Navy course says
tests have found body resistance "under unfavorable conditions" as low as
300 ohms, which at 120 volts lets 400 mA through (our arithmetic), and it
records fatalities from voltages as low as 30 volts. NIOSH says
resistance falls further when the contact is pressed harder or covers a
larger area, and OSHA's booklet explains that the current itself raises
blisters, which lower the body's resistance and let still more current
through. NIOSH's summary: "Of course, there is always a chance of
electrocution, even in dry conditions."

## How current hurts the body

OSHA lists four things that decide how bad a shock is: the amount of
current flowing through the body, the path it takes, how long the body
stays in the circuit, and the current's frequency.

**The amount.** OSHA and NIOSH both print the same table of what current
does when it flows from hand to foot for one second. Both credit it to a
1968 monograph by W. B. Kouwenhoven for the Instrument Society of America
(NIOSH also cites a 1973 paper), so it is restated here in our own words:

| Current, hand to foot, about one second | What it does |
| --- | --- |
| Below 1 mA | Usually not felt at all |
| 1 mA | A faint tingle |
| 5 mA | A slight shock: not painful, but disturbing. Most people can still let go, but a jerk away from it can cause other injuries |
| 6 to 25 mA (women), 9 to 30 mA (men) | A painful shock and loss of muscle control. This is the "let-go" range: the person cannot let go of what they are holding, although a strong contraction of other muscles can instead throw them clear |
| 50 to 150 mA | Extreme pain, breathing stops, severe muscle contractions. Death is possible |
| 1,000 to 4,300 mA | The heart's rhythmic pumping stops; muscles contract and nerves are damaged. Death is likely |
| 10,000 mA | Cardiac arrest and severe burns. Death is probable |

The Navy course gives the same picture in round numbers for 60 hertz
current, hand to hand or hand to foot: about 1 mA can be felt, about
10 mA stops you controlling your muscles, and about 100 mA (a tenth of an
ampere) is fatal if it lasts a second or more. It adds that the figures
are approximate "because individuals differ in their resistance to
electrical shock."

To put those currents in proportion, NIOSH notes that "a small power
drill uses 30 times as much current as what will kill."

**The time.** A longer shock is a worse shock. NIOSH's example: "a
current of 100 mA applied for 3 seconds is as dangerous as a current of
900 mA applied for a fraction of a second (0.03 seconds)." And the time
can stretch on its own. In the let-go range the muscles clamp, so a hand
holding a live tool or wire cannot release it; OSHA explains that this
"freezing" is extremely dangerous because it lengthens the exposure, and
the current raises blisters that lower the body's resistance and increase
the current. Hence the warning in OSHA's booklet on electrical hazards:
"Low voltage does not imply low hazard."

**The path.** NIOSH: "Currents through the heart or nervous system are
most dangerous." A current that enters one hand and leaves by the other
side of the body, or by the feet, crosses the chest. NIOSH notes that a
large number of serious electrical injuries involve current passing from
the hands to the feet, a path through both the heart and the lungs, and
that this kind of shock is often fatal.

**The frequency.** Household current in the United States alternates 60
times a second (below). OSHA lists frequency as one of the four factors,
and the Navy course's round numbers above are for 60 hertz current like
that.

## Power: watts

The Navy course: "Power in watts is equal to the voltage across a circuit
multiplied by current through the circuit." As a formula, power (watts) =
voltage (volts) x current (amps). A device's power rating is the rate at
which it turns electrical energy into something else: light, heat or
motion.

Energy is power used over time. The course's practical unit is the
watt-hour, one watt used continuously for one hour, and a kilowatt-hour
(kWh) is 1,000 of them: the unit on an electricity bill. For larger
motors you will meet horsepower, and the course gives one horsepower as
746 watts. [Your First Solar Power](first_solar_power.md) turns watts and
watt-hours into kitchen language.

**Worked example: amps from a label.** Rearrange the formula and current =
watts / volts. All the arithmetic here is ours:

- A 60 watt lamp on 120 volts draws 0.5 A.
- A 1,200 watt heater on 120 volts draws 10 A.
- A circuit protected at 15 amps is meant to carry at most 15 x 120 =
  1,800 watts, so a 1,200 watt heater and a 700 watt microwave on the same
  circuit (1,900 watts) already ask more of it than that.

Motors are the exception to reading a label at face value. NIOSH's
electrical manual says electric motors draw extra current as they start
or if they stall, "requiring up to 200% of the nameplate current rating."
A saw that binds in the cut, or a pump that seizes, can draw up to double
what its label says.

## Why current heats wires

Every conductor has some resistance, and current pushed through
resistance turns into heat. That is how a toaster works on purpose and
how a wire overheats by accident. Put the two formulas together (power =
voltage x current, and voltage = current x resistance) and the heat made
in a wire is its resistance times the current squared (our algebra).
Squared is the important part: **double the current and a wire makes
four times the heat.**

**What sets a wire's resistance.** The Navy course gives four things: the
material, the length (the longer the wire, the more resistance), the
cross-section (the thicker, the less), and the temperature (in a metal
like copper, resistance rises as it warms, as the table below shows).

Wire is sized in America by the American Wire Gauge (AWG), and NIOSH's
reminder is worth learning by heart: "The larger the gauge number, the
smaller the wire!" The National Bureau of Standards' copper wire tables
(the bureau is now NIST) give the resistance of standard annealed copper:

| Gauge (AWG) | Ohms per 1,000 feet at 20 C | Ohms per kilometre at 20 C | Ohms per kilometre at 75 C |
| --- | --- | --- | --- |
| 16 | 4.02 | 13.2 | 16.0 |
| 14 | 2.52 | 8.28 | 10.1 |
| 12 | 1.59 | 5.21 | 6.34 |
| 10 | 0.9988 | 3.277 | 3.985 |

Each step of two gauges thicker cuts the resistance by more than a third,
and a wire at 75 C has about a fifth more resistance than the same wire
at 20 C (our reading of the table), so a hot wire makes yet more heat
from the same current.

### Worked example: a long extension cord

A 1,200 watt heater draws 10 A at 120 volts. Run it through a 100 foot
extension cord. The current goes out along one wire and back along the
other, so it passes through 200 feet of copper (our arithmetic
throughout, from the table above):

| Cord | Resistance of 200 feet | Voltage lost in the cord | Heat made in the cord |
| --- | --- | --- | --- |
| 16 gauge | 0.80 ohm | 8.0 volts (6.7 percent) | 80 watts |
| 12 gauge | 0.32 ohm | 3.2 volts (2.7 percent) | 32 watts |

Spread along a cord lying loose in open air, that heat gets away. Coiled
up, or under a rug, it cannot (our explanation). The CPSC's home
electrical checklist warns that "Wrapped cords trap heat that normally
escapes loose cords," that rugs and furniture resting on cords can crush
their insulation, and that "Too much current will cause the wires to get
hot. If the cord, plug, or outlet feels warm, it may be overloaded, and
can be a fire hazard." The lost voltage matters too: NIOSH says that if a
cord is too long the voltage drop can be enough to damage equipment,
because many motors only operate safely in a narrow range of voltages.

The CPSC's checklist says to check the electrical rating on appliances
and on extension cords, and NIOSH says to check the tool manufacturer's
recommendations for the gauge and the length of cord. The published
figures for a 16 gauge cord differ (the CPSC's checklist says it handles
1,375 watts, NIOSH's table 13 amps), which is one more reason to read the
cord and the manual rather than a rule of thumb.

### Loose connections and old aluminium wiring

The CPSC's checklist explains why a warm outlet or switch is a warning:
it may mean a loose electrical connection, and "A loose connection cannot
carry much current without getting hot." The reason is the same
arithmetic. All of the circuit's current squeezes through the few points
where a loose screw or a worn contact actually touches, which is a small
resistance in one tiny spot, and the heat it makes stays in that spot
(our explanation).

The CPSC's booklet on aluminium wiring is that effect at the scale of a
whole house. Aluminium branch circuits were installed in American homes
mainly from the mid 1960s to the mid 1970s, during a copper shortage. The
CPSC describes connections to aluminium wire deteriorating over time in
ways that raise their resistance, and that raised resistance causing
overheating, sometimes at hazardous levels. A national survey for the
CPSC found homes built before 1972 and wired with aluminium were 55
times more likely than homes wired with copper to have at least one
outlet connection reach "Fire Hazard Conditions". The booklet's warning
signs are hot faceplates on outlets or switches, flickering lights,
circuits that do not work, and a smell of burning plastic at outlets or
switches, and its instruction is to have a qualified electrician find the
cause: "DO NOT TRY TO DO IT YOURSELF."

## What breakers, fuses and GFCIs protect

**Breakers and fuses protect wires.** OSHA: "Fuses and circuit breakers
are designed to protect conductors and equipment." They open the circuit
when the current gets high enough to overheat the wiring. NIOSH is
blunter: "Overcurrent protection devices are designed to protect
equipment and structures from fire. They do not protect you from
electrical shock!" Its numbers make the point. "Death can result from
20 mA (.020 amps) through the chest," while the lowest overcurrent at
which NIOSH says a typical fuse or breaker opens is 15 amps. Fifteen amps
is 750 times 20 mA (our arithmetic). A breaker never notices the current
that kills a person.

That is also why a bigger fuse or breaker is never a fix. NIOSH notes
that a circuit's breaker is normally matched to its wire size; a bigger
one lets more current into the same wire, and twice the current makes
four times the heat. The CPSC says the wrong size of fuse "can allow too
much current to flow and cause the wiring to overheat, creating a fire
hazard," and NIOSH gives the example of "using a 30 amp fuse in a 20 amp
circuit" as dangerous.

**A ground-fault circuit interrupter (GFCI) protects people.** It watches
the current going out along one wire and the current coming back along
the other. In a healthy circuit they are equal. If they differ, some
current is leaving by another path, perhaps through a person to the
ground, and the GFCI switches off. NIOSH puts the trip point at a leak of
as little as 4 to 6 mA; OSHA says 5 milliamperes, within as little as
1/40 of a second; the CPSC says as little as 0.006 amperes. That is
within the "slight shock" end of the table above, which is the point:
the CPSC writes that GFCIs "are designed to operate before the
electricity can affect your heartbeat."

A GFCI has limits. NIOSH warns that it "does not protect a person from
line-to-line hazards", meaning touching two live wires, or a live wire
and the neutral, at the same time. In that case the current goes out
along one wire and back along the other through your body, the two
currents still match, and the GFCI sees nothing wrong (our explanation of
NIOSH's point). NIOSH adds that a shock may still be felt even when a
GFCI trips, and that your reaction to it can cause a fall. And a GFCI only
works if it works: the CPSC says to test each one at least once a month,
and [Working Out Why Something Broke](working_out_why_something_broke.md)
gives its lamp test.

**An arc-fault circuit interrupter (AFCI)** is a breaker that, in the
CPSC's description, detects the electrical arcing that can happen when a
wire or connection is damaged, before it causes unnoticed overheating and
a fire. The CPSC says to test AFCIs monthly too.

**Grounding** is a deliberate low-resistance path to the earth. OSHA
explains the equipment ground, the third wire and the third pin on a
plug: if a fault inside a tool makes its metal frame live, the ground
wire gives that current a second path to the ground, and the resulting
flow "may activate the circuit protection devices," so the breaker can
trip instead of the case staying live and waiting for a hand (our
wording). OSHA is careful to add that grounding "is normally a secondary
protective measure" and "does not guarantee that you won't get a shock".
A double-insulated tool, marked as such, protects by a second layer of
insulation instead of a ground wire.

## Alternating current and the house supply

A battery pushes current one way, steadily: direct current (DC). The
supply in a house reverses direction many times a second: alternating
current (AC). The Navy course gives the American supply as "120-volts,
60 Hz": 60 hertz means 60 complete cycles every second.

The "120 volts" is not the highest voltage in the cycle. It is what the
Navy course calls the effective value (also called the root mean square,
or rms): the steady voltage that would make the same heat in a resistor.
The course gives the peak of a sine wave as 1.414 times the effective
value, so a 120 volt supply peaks at about 170 volts in each direction
(our arithmetic).

**Two live wires, and 240 volts.** Some appliances in a house, such as a
cooker or a water heater, run on 240 volts (general knowledge), and
NIOSH explains where that comes from: "one live wire may be at +120
volts while the other is at -120 volts during an alternating current
cycle--a difference of 240 volts." Touching both of those is a 240 volt
shock, and a GFCI will not see it (above).

**The colours of the wires.** In NIOSH's description, "In most household
wiring, the black wires and the red wires are at 120 volts," white wires
are connected to ground and at 0 volts, and OSHA adds that equipment
grounding wires are green, or green with yellow stripes. But the words
that matter are "most" and "usually". NIOSH tells of a maintenance worker
who removed the fuse from the black wire of a 277 volt light, believing
it was the live one, and was killed stripping the white wire, which had
been made live by a mistake in installation. A colour tells you what the
installer intended, not what is there. The procedure for finding out what
is there, with a voltage tester, is in [Working Out Why Something
Broke](working_out_why_something_broke.md).

**Why one wire is connected to the earth.** OSHA describes the system
ground: the neutral wire is grounded at the transformer and at the
building's service entrance. That is why a live wire is dangerous to
anyone touching anything connected to the earth, a tap, a metal pipe or a
wet floor (our explanation): NIOSH notes that plumbing is often grounded,
and that touching a live wire while you are in contact with any grounded
object will give you a shock. A bird sitting on one power line has both
feet at the same voltage, so no current flows through it; a person
holding a ladder that touches the same line, with their feet on the
ground, is a path to the earth (general physics).

## Static and lightning

**Static electricity** is charge that builds up on a surface and jumps
when it can, as when you reach for a door knob on a cold dry day. OSHA's
booklet says it is generally not as severe a shock as current from a
circuit, but that friction can build enough static on plastic pipe,
materials or rubber drive belts for a discharge to ignite flammable or
combustible vapours or dusts nearby, and that grounding or other measures
may be needed to prevent the build-up.

**Lightning** is the same physics at an enormous scale. The
National Weather Service: "A typical lightning flash is about 300 million
Volts and about 30,000 Amps." It compares that with household current, 120
volts and 15 amps. OSHA's point about air applies: an insulator becomes a
conductor during a lightning stroke. What to do about it, beginning with
going indoors when you hear thunder, is in [Reading the
Weather](reading_the_weather.md).

## Batteries and low-voltage DC

Ohm's law explains why a 12 volt car battery rarely gives a noticeable
shock through dry hands: 12 volts across 100,000 ohms is about 0.12 mA
(our arithmetic). It does not make a battery safe. A battery can deliver
an enormous current through anything with very little resistance, and a
spanner across the terminals or a ring or watch strap against one is
exactly that: the metal heats in an instant and burns the skin around it
(general practice). NIOSH's advice for anyone working near high currents
is to remove jewellery and other metal objects first, because they "can
cause burns if worn near high currents". Battery banks for solar power
can be built at 48 volts and more, and the Navy course's record of deaths
from 30 volts applies to them. [Batteries and
Storage](batteries_and_storage.md) covers them, including fire.

## Know where your own work stops

- **Measuring a live circuit.** OSHA allows only qualified persons to do
  testing work on electric circuits at work, and NIOSH lists taking
  voltage and current measurements among the tasks that put you near live
  parts. The one test a household should learn, proving a circuit dead
  before touching it, has its own procedure in [Working Out Why Something
  Broke](working_out_why_something_broke.md); without a tester, or without
  being shown how to use one, it is not yours to do.
- **Anything inside a panel, any new wiring, and any repair to wiring:**
  [Where Your Own Electrical Work Stops](where_your_electrical_work_stops.md)
  draws those lines, with the permits that come with them.
- **A warm outlet, a burning smell, flickering lights or a shock from an
  appliance:** a qualified electrician, as the CPSC says above.
- **Anyone who has had an electric shock:** medical care, every time
  (OSHA).

## Traps

- **"It's only 120 volts."** The Navy course records deaths from 30
  volts, and the worked example above shows 120 volts across wet skin
  passing a current that can stop breathing.
- **"The breaker will cut it off."** A breaker opens at amps; a person
  dies at milliamps.
- **"A bigger fuse will stop it blowing."** It will, by letting the wire
  overheat instead.
- **"The white wire is safe."** Usually, until it is not. Test.
- **"The switch is off."** A switch can be wired wrongly. NIOSH tells of
  a furnace technician killed by a toggle switch that let power through
  in the "off" position. A switch is a control, not proof.
- **"I'm wearing rubber soles."** NIOSH: "Tennis shoes will not protect
  you from electrical hazards."
- **"It has a GFCI, so I can't be shocked."** Not between two wires, and
  not if the GFCI has failed without anyone testing it.
- **"Any old cord will do."** A thin cord on a big load turns current
  into heat and wastes voltage the tool needed.

## How the game models it

The game models electricity as an engineer sizing cables would, and
nothing in it can shock you.

- **Sizing a cable on the Construction page.** When you connect two
  machines with a power run, the Buildability section's Conduits check
  works out the current the way this guide does: watts divided by volts,
  at a fixed 120 volts. It takes the resistance of the copper per metre,
  counts the run out and back over the straight-line distance between the
  two machines, and works out the voltage lost. A run passes when its
  current is at most 80 percent of the cable's rating and it loses at most
  3 percent of the voltage, earns a warning up to the full rating or 5
  percent, and fails beyond either, or if the cable's voltage rating is
  below 120 volts. You can leave a run on "auto", which picks the cheapest
  cable that passes, or pin a cable and see whether it holds. The check is
  `check_cable` in `src/utilities.rs`, called from `src/machines.rs`.
- **The cables are real copper.** The catalogue in
  `data/utilities/conduits.ron` lists 14, 12 and 10 gauge copper at 15,
  20 and 30 amps for the home, an industrial 6 gauge at 55 amps, and a
  room-temperature superconductor that the game treats as future
  technology. The game's resistance figures match the National Bureau of
  Standards' table above at 20 C (our comparison), and its 15 and 20 amp
  pairings for 14 and 12 gauge copper are the ones the CPSC's aluminium
  wiring booklet gives for house circuits.
- **A worked example with the game's numbers.** A 1,000 watt machine
  draws 8.3 amps. Twenty metres from its supply, on 14 gauge, it loses
  2.8 volts, 2.3 percent: a pass. Move it to 40 metres and 14 gauge loses
  4.6 percent, a warning, so "auto" chooses 12 gauge instead, which loses
  2.9 percent. A 3,000 watt machine draws 25 amps, more than 14 or 12
  gauge is rated for, so it fails on either; on 10 gauge it is inside the
  30 amp rating but over the 80 percent line, a warning, so on a short run
  "auto" picks the industrial 6 gauge. (Our arithmetic, following the
  game's rule.)
- **The running electrical simulation counts watts.** While you play,
  each wired circuit balances the watts its generators make against the
  watts its machines use, and its batteries fill and empty in
  watt-hours (`src/systems/electrical.rs`). The HUD's Power line shows
  generation, use and the net in watts, and its Battery line the charge
  in kWh and the hours it would last. In the default Station-supplied
  life support mode the ship's reactor makes up any shortfall; in the
  Realistic mode, a circuit that cannot carry its load switches its
  machines off, the optional ones first and the critical ones last.

What the game leaves out, so you do not learn it from the game: there is
no current through a body and no shock, burn or arc; no alternating
current, frequency or peak voltage; no 240 volt circuits; no ground, no
breakers, fuses, GFCIs or AFCIs; and a cable that fails the check is only
a failed line in a report, never a hot or burning wire. The resistance
never changes with temperature. Code for lightning damage exists in
`src/systems/disasters.rs`, but it is not running in the game.

## You own this when

- You can say what volts, amps, ohms and watts each measure, and move
  between them with Ohm's law and power = volts x amps.
- You can work out from a label how many amps a device draws, and how
  many watts a 15 amp circuit can carry.
- You can explain why wet skin turns a tingle into a deadly current, and
  why "it's only 120 volts" is wrong.
- You know the four things that decide how bad a shock is: how much
  current, which path, how long, and what frequency.
- You can explain why double the current means four times the heat, and
  why a long thin cord, a coiled cord and a loose connection all get hot.
- You know that breakers and fuses protect wires and GFCIs protect people,
  and what a GFCI cannot do.
- You never trust a wire's colour or a switch's position in place of a
  tester.
- You would not touch someone who is being shocked, and you know what to
  do instead.

## Sources

Grouped by what kind of authority each one is. Every source here is a
work of the United States federal government, and its own text is in the
public domain. Documents were read on 4 October 2026; regulations were
read in the eCFR's text in force on 1 October 2026.

### United States government (public domain)

- US Navy. *Navy Electricity and Electronics Training Series, Module 1,
  Introduction to Matter, Energy, and Direct Current*, NAVEDTRA 14173,
  September 1998, Distribution Statement A, approved for public release
  (current as a directed movement of electrons; the ampere as one
  coulomb a second; the water-tank comparison for a difference of
  potential; the ohm; the four things that set a conductor's resistance;
  Ohm's law; power equals voltage times current; the watt-hour and
  kilowatt-hour; one horsepower as 746 watts; current, rather than
  voltage, the measure of shock intensity; about 1, 10 and 100
  milliamperes at 60 hertz; body resistance as low as 300 ohms under
  unfavourable conditions; fatalities recorded from 30 volts). Copy
  hosted in the Internet Archive's NEETSModules collection; parts of its
  text layer use a shifted font encoding and were decoded before
  reading. https://archive.org/details/NEETSModules
- US Navy. *Navy Electricity and Electronics Training Series, Module 2,
  Introduction to Alternating Current and Transformers*, NAVEDTRA 14174,
  September 1998, Distribution Statement A (the nominal US household
  supply of 120 volts at 60 Hz, given in an assignment question; the
  effective or rms value as the heating equivalent of direct current; the
  peak as 1.414 times the effective value). Same collection.
  https://archive.org/details/NEETSModules
- National Bureau of Standards. *Copper Wire Tables*, Handbook 100,
  issued 21 February 1966 (tables 5, 6 and 11: resistance of standard
  annealed copper by American Wire Gauge, in ohms per 1,000 feet and per
  kilometre, at 0 to 200 C). The National Bureau of Standards is now the
  National Institute of Standards and Technology.
  https://nvlpubs.nist.gov/nistpubs/Legacy/hb/nbshandbook100.pdf
- National Institute of Standards and Technology. CODATA value,
  elementary charge, 2022 CODATA recommended values (1.602 176 634 x
  10^-19 coulomb, exact).
  https://physics.nist.gov/cgi-bin/cuu/Value?e
- Occupational Safety and Health Administration. *Controlling Electrical
  Hazards*, OSHA 3075, 2002 (Revised) (conductors and insulators, the
  earth, air in an arc or lightning; water and skin; the four ways the
  body completes a circuit; the four factors in a shock; the table of
  effects of current, which OSHA credits to W. B. Kouwenhoven, "Human
  Safety and Electric Shock," Instrument Society of America, 1968,
  restated here; burns the most common shock-related injury and their
  three kinds; freezing, blisters and "Low voltage does not imply low
  hazard"; hidden internal injuries and emergency medical help; static
  electricity; wire colours; system and equipment grounding as a
  secondary measure; fuses and breakers protect conductors and
  equipment; GFCIs at 5 milliamperes within 1/40 of a second; arc-fault
  devices). https://www.osha.gov/sites/default/files/publications/osha3075.pdf
- Occupational Safety and Health Administration. *Hand and Power Tools*,
  OSHA 3080, 2002 (Revised) (a shock can cause a fall from a ladder or
  elevated surface).
  https://www.osha.gov/sites/default/files/publications/osha3080.pdf
- Occupational Safety and Health Administration. 29 CFR 1910.334(c)(1)
  (only qualified persons may perform testing work on electric circuits
  or equipment). https://www.ecfr.gov/current/title-29/subtitle-B/chapter-XVII/part-1910/subpart-S/section-1910.334
- National Institute for Occupational Safety and Health. *Electrical
  Safety: Safety and Health for Electrical Trades, Student Manual*, DHHS
  (NIOSH) Publication 2009-113 (supersedes 2002-123), April 2009 (wet
  clothing, humidity and perspiration; plugging into grounded plumbing;
  black and red wires usually at 120 volts and white at 0; the 240 volt
  cable; the 277 volt lamp with a live white wire; the furnace switch
  wired to let power through when off; the shock table with 15 amps as
  the lowest overcurrent at which a typical fuse or breaker opens; 100 mA
  for 3 seconds against 900 mA for 0.03 seconds; a small power drill
  using 30 times the current that kills; dry and wet skin resistance,
  and harder or larger contact lowering it; currents through the heart or
  nervous system most dangerous, and hands to feet; arc blasts, their
  temperature, pressure wave and molten metal, often from equipment
  failure; even a mild shock causing a fall; overloads a major cause of
  fires; the larger the gauge number, the smaller the wire; motor
  starting current up to 200 percent; voltage drop in long cords and the
  manufacturer's gauge and length; breakers matched to wire size;
  overcurrent devices do not protect people, 20 mA through the chest; the
  30 amp fuse in a 20 amp circuit; GFCIs at 4 to 6 mA, a shock still
  felt, and their line-to-line limit; removing jewellery near high
  currents; taking measurements as work near live parts; first aid;
  tennis shoes). Read in a copy hosted at safety.duke.edu and in the CDC's
  own PDF, fetched through a page reader because cdc.gov refuses scripted
  downloads; the wording quoted here is in both.
  https://www.cdc.gov/niosh/docs/2009-113/pdfs/2009-113.pdf
- US Consumer Product Safety Commission. *Home Electrical Safety
  Checklist*, Publication 513, July 2008, prepared by CPSC staff (wrapped
  cords trap heat; cords under rugs and furniture; too much current
  makes wires hot, a warm cord, plug or outlet; 16 gauge cords and 1,375
  watts; warm outlets and loose connections; the wrong size of fuse; AFCIs
  and testing them monthly). https://www.cpsc.gov/s3fs-public/513.pdf
- US Consumer Product Safety Commission. *Repairing Aluminum Wiring*,
  Publication 516, June 2011 (aluminium branch wiring from the mid 1960s to
  the mid 1970s; deterioration raising the resistance of connections and
  causing overheating; homes before 1972 55 times more likely to have an
  outlet connection reach "Fire Hazard Conditions"; the warning signs;
  copper 14 and 12 gauge for 15 and 20 amp circuits; "DO NOT TRY TO DO IT
  YOURSELF"). https://www.cpsc.gov/s3fs-public/516.pdf
- US Consumer Product Safety Commission. *What Is a GFCI?*, Publication
  099, printed with the code 092010 (as little as 0.006 amperes; designed
  to operate before the electricity can affect your heartbeat; monthly
  testing). https://www.cpsc.gov/s3fs-public/099_0.pdf
- National Weather Service. How Powerful Is Lightning?, undated (about
  300 million volts and 30,000 amps in a typical flash, against household
  current of 120 volts and 15 amps).
  https://www.weather.gov/safety/lightning-power

### Inside this project

- The cable check: `check_cable` and `cheapest_cable_for` in
  `src/utilities.rs`, run by the Conduits check in `src/machines.rs`
  (fixed 120 volts; the 80 percent, 3 percent and 5 percent limits) and
  shown in the Buildability section and the cable picker of the
  Construction page (`src/gui/pages/construction.rs`). The cable
  catalogue: `data/utilities/conduits.ron`.
- The running electrical simulation: `src/systems/electrical.rs`, with the
  ship's reactor feed in `src/systems/ship_power.rs`; the HUD's Power and
  Battery lines in `src/gui/pages/hud.rs`; the life support setting in
  `src/config.rs`. The unused lightning damage: `src/systems/disasters.rs`
  (its `DisasterSystem` is not registered with the game's system runner).
- [Where Your Own Electrical Work Stops](where_your_electrical_work_stops.md),
  [Working Out Why Something Broke](working_out_why_something_broke.md),
  [Power Tools](power_tools.md), [Your First Solar
  Power](first_solar_power.md), [Batteries and
  Storage](batteries_and_storage.md), [Treating Burns](treating_burns.md),
  [Roofs and Keeping Water Out](roofs_and_keeping_water_out.md) and
  [Reading the Weather](reading_the_weather.md).

### Labelled in the text as arithmetic, general physics or our explanation, not sourced

- Every worked calculation: electrons per second in an ampere, current
  through dry, wet and 300 ohm skin, amps from watts, the 1,800 watt
  circuit and the heater with the microwave, the extension cord, the 750
  times between a breaker and a lethal current, the 170 volt peak, the 12
  volt battery, and the game examples.
- That heat in a wire goes with the square of the current (algebra from
  the two formulas the Navy course gives), and the reading of the copper
  table (a step of two gauges, the effect of 75 C).
- Why heat escapes a loose cord and not a coiled one, why a loose
  connection heats one spot, why a live wire is dangerous to anyone
  touching a grounded object, and why a GFCI cannot see a line-to-line
  shock.
- That a voltage is stated between two points, often a wire and the
  ground (our wording of the Navy course's point).
- The bird on a power line, and a ring or spanner across a battery's
  terminals.
- That arcs are one reason the inside of a panel is an electrician's
  work, and the comparison of the game's cables with the copper table.
