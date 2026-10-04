# Working Out Why Something Broke

When something stops working, the tempting move is to replace the part
that seems most likely and see if that fixes it. Sometimes it does. When
it does not, you replace the next most likely part, and the next, until
the thing works or the money runs out. It is slow, it is expensive, and
even when it works it can leave the real cause in place, so the new
part fails the same way a few months later.

Diagnosis is the alternative: a method for narrowing down where the
fault is, by tests that each rule out part of the machine, until there
is only one place left it can be. Then you fix that, and then you ask
why it failed, so it does not happen again.

The method in this guide comes from the US Navy's handbook for its
electronics technicians, and the vocabulary for causes comes from NASA's
rules for investigating accidents. The safety rules come from the
Occupational Safety and Health Administration (OSHA) and the same Navy
handbook, and the household examples come from the Environmental
Protection Agency (EPA) and the Consumer Product Safety Commission
(CPSC). All are United States government publications, in the public
domain. Where something is general practice rather than a published
rule, the text says so.

## First, make it safe to look at

The danger in repair work is a machine that is not as dead as the
person working on it thinks: one that starts up unexpectedly, or
releases energy it was still storing. Before you open, touch or reach
into anything, take away the energy that can hurt you.

OSHA's rule on controlling hazardous energy (29 CFR 1910.147, often
called lockout/tagout) is written for workplaces, and it does not even
cover farm employment; nobody enforces it in your shed. Its logic is
still the right logic for any repair at home:

- **Isolate the machine from every energy source.** OSHA's list of
  energy sources is broad: electrical, mechanical, hydraulic, pneumatic,
  chemical, thermal, or other energy. A water pump has electricity and
  water pressure. A log splitter has fuel, hydraulic pressure and a
  heavy ram. Find all of them.
- **Use a real isolator, not the on/off button.** OSHA says plainly that
  push buttons, selector switches and other control devices are not
  energy isolating devices. An isolator is a breaker, a disconnect, a
  valve, or a block, something that physically stops the energy. A
  machine switched off at its own button can be switched on again by a
  fault, a timer, or another person.
- **For anything with a plug, unplug it and keep the plug where you
  control it.** OSHA's rule treats corded equipment as safely isolated
  when it is unplugged and the plug stays under the exclusive control of
  the person working on it. In practice: in your hand, or in sight and
  out of anyone else's reach.
- **Lock or label the isolator.** At home this can be as simple as a
  piece of tape over the breaker and a note saying who is working on
  what. That is general practice, the household version of the lock or
  tag OSHA requires at work.
- **Release the energy that stays behind after you switch off.** OSHA
  requires that all potentially hazardous stored or residual energy be
  relieved, disconnected, restrained, or otherwise rendered safe. Stored
  energy is the part people forget: a pressure tank still full of water
  under pressure, a spring under tension, a raised part held up only by
  hydraulics, a hot engine. The Navy's handbook adds the electrical one:
  capacitors retain an electrical charge, and it says to discharge all
  capacitors and circuits containing them before working on them.
- **Check that it is actually dead before you start.** OSHA requires the
  person to verify that isolation and de-energisation have been achieved
  before starting work. Its sample procedure does it this way: first make
  sure nobody is exposed, then operate the machine's normal controls to
  make certain it will not run, then return the controls to off.

The Navy's handbook opens with a list of habits for anyone working on
electrical equipment, and several translate directly to home repair:
never work alone; do not work on energised equipment unless absolutely
necessary; never attempt to repair energised circuits except in an
emergency; use only one hand when operating circuit breakers or
switches; and never bypass an interlock without authority.

**Know where your diagnosis stops.** The CPSC's advice on ground-fault
outlets is a good model: test them yourself, but if one fails its test
or you are in doubt about wiring, contact a qualified electrician. Fuel
gas, the inside of an electrical panel, and anything whose failure could
hurt somebody other than you are places where your diagnosis can end at
"it is in there somewhere, and I am calling someone".

## The six steps

The US Navy's electronics technicians' handbook (NEETS Module 19) says
that even a complex repair can be broken into simple steps, and that any
repair should follow them in order. They were written for electrical
and electronic equipment. That they carry over to a pump, a mower or a
leaking pipe is our reading, and the worked examples below show how.

1. **Symptom recognition.** Noticing that something is wrong.
2. **Symptom elaboration.** Getting a more detailed description of the
   trouble.
3. **Listing probable faulty functions.** Given what you have learned,
   which parts of the system could logically be at fault?
4. **Localising the faulty function.** Finding which of those parts
   actually is at fault.
5. **Localising the trouble to the circuit.** Testing further, inside
   that part, to find the specific fault.
6. **Failure analysis.** Working out which component is faulty,
   repairing or replacing it, determining what caused the failure,
   returning the equipment to proper operation, and recording what you
   did for whoever maintains it next. The handbook adds that you should
   reorder any parts you used.

The tempting shortcut is to jump from step 1 straight to replacing a
part. The steps in between are where the work is, and they are cheap:
they are mostly looking, thinking and simple tests.

## Steps 1 and 2: describe the symptom properly

"The pump is broken" is a recognition. It is not yet a description.
Before touching a tool, answer these:

- **What exactly happens?** Does the pump hum but not turn, turn but not
  pump, pump but not build pressure, or do nothing at all? Each of those
  points somewhere different.
- **What still works?** It is easy to forget to ask, and the answer is
  free evidence. If the lights in the pump house work, the supply to the
  building is fine. Everything that still works rules something out.
- **When did it start, and what changed just before?** A new part, a
  move, a storm, a cold night, a different fuel, a repair last week.
  Treating the most recent change as the first suspect is general
  practice, and a good one, because it is usually quick to check.
- **Is it constant or intermittent?** If it comes and goes, when?
  Morning or evening, hot or cold, wet or dry, under load or idle?
- **Has it happened before?** Here a maintenance log pays for itself;
  see [Keeping Records](keeping_records.md).

Use your senses as instruments. The Navy's handbook tells its
technicians to observe the equipment's operation for any and all faults
and to check for defective components with their eyes and nose. A burnt
smell, a discoloured part, a wet patch, a new noise or a hot spot is
evidence. Write it down before you start changing things, because the
first thing you change destroys some of it.

**Intermittent faults.** A fault that has gone away has not been fixed.
If you cannot make it happen on demand, keep a log of each time it does:
date, time, conditions, what you were doing. Patterns appear in the
log that nobody would see from memory.

## Step 3: what could cause this?

Draw the system as a chain of functions, each one feeding the next.
A well pump system, for example, is something like: power supply,
switch, pump motor, pump, pipe, pressure tank, pressure switch, taps.
Water flows one way through it and control signals go round it.

Then ask which links in the chain could produce exactly the symptom
you described. "The pump runs but no water comes out" rules out the
power supply and the motor, because they are working. It points at the
pump itself, the pipe, or a valve. This is step 3: from the symptom,
list what could logically be at fault, and just as importantly, what
cannot.

## Steps 4 and 5: narrow it down

Now test, and choose each test to rule out as much as possible.

### Test at the boundaries

The most useful test checks whether the right thing is arriving at a
point in the chain. If water arrives at the pressure tank but not at the
tap, the fault is downstream of the tank. If power arrives at the motor
but the motor does not turn, the fault is in the motor or what it
drives. Each boundary test cuts the chain in two.

### Split in the middle when you have no better idea

If you have no strong suspect, test as near the middle of the chain as
you can. Whatever the answer, you have ruled out half the chain. Test
the middle of the half that is left, and so on. The arithmetic is what
makes this powerful: a chain of 16 parts takes at most 4 tests to reach
one part (16, 8, 4, 2, 1), where testing one part at a time could take
15. Twice as many parts costs only one more test.

### Start with what is likely and easy

Splitting in the middle assumes every part is equally likely to have
failed and equally easy to test, which is rarely true. Where you know
better, use it. Check the cheap, quick things first: is it plugged in,
is the breaker on, is there fuel, is the valve open, is the filter
blocked. These sound insulting, and they are the cheapest tests there
are. Doing them first is general practice rather than a published
rule: a check that costs a minute is worth doing before one that costs
an afternoon, whatever the odds.

### Substitute something known to be good

If you can swap a suspect part for one you know works, the swap is a
test: if the fault moves with the part, you have found it. The two
conditions are that the substitute really is known to be good, and that
the swap cannot damage it. A good battery put into a device with a short
circuit can be ruined by it, and a new fuse put into a circuit with a
fault will blow again. A fuse exists to blow when too much current
flows, so a fuse that blows again after replacement is evidence rather
than the fault: whatever drew the current is somewhere else.

### Change one thing at a time

If you change three things and the fault disappears, you do not know
which one fixed it, and you may have introduced a new problem with one
of the other two. Change one thing, test, and write down the result.
If a change did not help, consider putting it back, so that the system
is still in a known state. General practice, and the reason is the same
as for any experiment: one variable at a time.

### Check that a new part is actually good

New parts fail too. If a replacement does not fix the fault, do not
assume the diagnosis was wrong until you have checked the new part.

## Step 6: fix it, then ask why it failed

When you find the faulty part, the Navy's handbook asks for more than
replacing it: determine what caused the failure. Its list of hints says
the same thing again, to analyse the cause of the failure for a possible
underlying problem. A part that failed because it was old is a
maintenance question. A part that failed because something else
stressed it will fail again, because the something else is still there.

### The vocabulary of causes

NASA's rules for investigating mishaps (NPR 8621.1) use a precise set of
terms that are worth borrowing, because they stop you from settling for
the first answer.

- **Proximate cause**, also called the direct cause: the event, and the
  conditions just before it, that directly produced the failure, and
  that if removed would have prevented it.
- **Intermediate cause**: something earlier that directly produced the
  proximate cause.
- **Root cause**: an earlier event or condition, which NASA says is
  primarily associated with organisational factors, that led to the
  intermediate cause and, if removed or changed, would have prevented
  the failure. NASA notes that typically several causes contribute to
  an undesired outcome.
- **Contributing factor**: something that may have contributed, but
  whose removal would not on its own have prevented the failure.

NASA's guidance is that root cause analysis should continue until
organisational factors have been found, or until the data run out. At
home, "organizational factors" means habits, schedules and who is
responsible for what. That translation is our reading, but it is the
useful one: the root cause of a household breakdown is often that
nobody was checking the thing that wore out.

**A worked example.** A toilet runs on and off by itself. The proximate
cause is a flapper that no longer seals. The EPA explains why: the
flapper is a rubber part, rubber wears out, and an old or worn flapper
can make a toilet flush on its own or silently leak thousands of gallons
a year. Replacing it fixes the toilet. But the EPA also says the flapper
should be checked periodically and replaced at least every five years.
If nobody in the house knew that, the root cause in NASA's sense is the
missing check, and the full fix is a new flapper plus a line in the
maintenance log with a date to look at it again.

The same structure, at much larger scale, is in [Units and Converting
Them](units_and_converting_them.md), which retells how a spacecraft was
lost at Mars to a unit mismatch, and how its investigators separated the
root cause from the contributing causes and the checks that should have
caught it.

### Then put it back and test it

Returning the equipment to proper operation is part of the Navy's
step 6. Reassemble, restore the energy you isolated, and test it under
the conditions that produced the fault, not only on the bench. An
intermittent fault that showed up under load has not been shown to be
fixed by a test without load.

### And write it down

The last part of the Navy's step 6 is recording what you did, for the
next person who maintains the equipment, and the next person is often
you. Date, symptom, what you tested and found, what you replaced, and
what you think caused it. [Keeping Records](keeping_records.md) covers
how to keep that log.

## Worked examples

### Water that disappears

Your water bill is higher than usual, or you hear water running when no
tap is on.

1. **Confirm there is a leak at all.** The EPA's test: read the water
   meter, use no water for two hours, and read it again. If the reading
   changed at all, you probably have a leak. This is a boundary test on
   the whole house at once.
2. **Check the common sources.** The EPA names worn toilet flappers,
   dripping faucets and other leaking valves as common types of
   household leak.
3. **Test the toilets.** The EPA's dye test: put a few drops of food
   colouring in the toilet tank and wait 10 minutes. If colour appears
   in the bowl, the tank is leaking into it. Flush afterwards so the
   colouring does not stain the tank.
4. **Look at the faucets, showerheads and fittings.** The EPA suggests
   checking faucet gaskets and pipe fittings for water on the outside of
   the pipe, and checking garden hoses where they connect to the spigot.
5. **Split the house.** If your plumbing has shut-off valves for
   sections (one for the outdoor taps, one per toilet, one for the water
   heater), close one section and repeat the meter test. If the meter
   stops moving, the leak is in that section. This is splitting in the
   middle, applied to pipes, and it is our suggestion rather than the
   EPA's.

The EPA also offers a baseline check: in a cold month, when nobody is
watering, a family of four using more than 12,000 gallons could have
serious leaks. That comparison is only possible if you have kept your
bills or meter readings, which is a reason to keep them.

### An outlet that has gone dead

A bathroom outlet has stopped working.

1. **Elaborate the symptom.** Is it only this outlet? Do the lights in
   the room work? Other outlets in the room? Elsewhere in the house? A
   whole house dead points one way, one room another, one outlet a third.
2. **Check the breakers.** If a whole circuit is dead, look first for
   a breaker that has tripped.
3. **Look for a ground-fault outlet upstream.** The CPSC explains that a
   ground-fault circuit interrupter (GFCI) outlet protects whatever is
   plugged into it and also other outlets further downstream on the same
   circuit. So a dead bathroom outlet can be the result of a GFCI that
   tripped somewhere else, in another bathroom, a garage, a kitchen or
   outdoors. Find it and press its reset button.
4. **If it trips again at once, believe it.** The CPSC describes a GFCI
   as cutting the power when the current flowing into a circuit differs
   from the current returning by as little as 0.006 amperes: some of the
   current is going somewhere it should not, and the CPSC notes that
   ground faults most often happen when equipment is damaged or
   defective. The general-practice next step is a form of the
   one-thing-at-a-time rule above: unplug everything on that circuit
   and reset again. If it now holds, plug things back in one at a time; the one
   that trips it is the suspect, and it stays unplugged until it has
   been checked or replaced. If it trips with nothing plugged in, the
   fault is in the wiring or the device, and that is a job for a
   qualified electrician.

The CPSC's maintenance advice turns this from a repair into a habit:
test every GFCI after installation, at least once a month, and after a
power failure. Its test: plug in a lamp, switch it on, press the test
button, and the lamp should go out; press reset and it should come back.
If it does not go out, the CPSC says the GFCI is not working or not
correctly installed and to call a qualified electrician; if it does not
come back on after reset, replace the GFCI.

### A torch that will not light

A battery torch is the whole method in your hand, and it needs no
source beyond itself.

- The chain: batteries, contacts and springs, switch, bulb or LED.
- What still works? If you have another device that takes the same
  batteries and works, you have a source of known-good batteries.
- Substitute: put the known-good batteries in the torch. If it lights,
  the old batteries were the fault. If not, the batteries are ruled out
  and you have halved the problem.
- Look: green or white crust on the contacts is corrosion, and a gentle
  clean may be the whole repair.
- Then ask why. Batteries left in a torch in a drawer for years, or a
  torch left switched on, are root causes in the household sense used
  above: habits, not parts.

### An engine that will not start

The general practice for a small petrol engine that will not start is
to split by the four things it needs to run: fuel, air, a spark, and
compression. Each can be checked separately, so the first split is
cheap. Start with the easiest: is there fuel, and is it fresh; is the
air filter clean; is the fuel valve open and the choke set. The
manufacturer's manual for your engine gives its own test sequence and
safety steps; use it rather than a general list. Fuel and sparks
together are a fire, so do any spark check away from spilled or open
fuel, with the engine cool.

### A plant that is failing

Plants have their own version of this method in [Pests and
Disease](pests_and_disease.md), which starts by asking whether anything
is wrong at all and then uses the pattern of the damage (one plant or
all of them, one side of the bed or the whole garden) to tell living
causes from non-living ones before reaching for any treatment. It is
the same steps 2 to 4, applied to something alive.

## Traps

- **Stopping at the first plausible cause.** A cause that fits the
  symptom is a suspect, not a conviction. Test it.
- **Believing the part is new, therefore good.** Check it.
- **Changing several things at once.** You lose the ability to tell
  which one mattered.
- **Fixing the proximate cause only.** The fuse blows again, the
  flapper wears out again, the bearing that ran dry fails again. Ask the
  step 6 question.
- **"It fixed itself."** An intermittent fault that has stopped is
  still there. Log it.
- **Working on it live because it is quicker.** It is quicker until the
  one time it is not.

## How the game models it

The game does not ask you to diagnose a fault. It does show three
things that are worth recognising from this guide.

- **The Construction page runs the boundary tests for you.** Its
  Buildability section checks your home's systems one function at a
  time, and names the one that fails: whether there is a power source
  for the load ("Power source"), whether a day's generation covers a
  day's use ("Energy balance"), whether every connection points at a
  machine that exists ("Wiring"), whether each power cable is big enough
  for its load and length ("Conduits", sized for at most 5 percent
  voltage drop at 120 volts), whether data links carry their traffic
  ("Data links"), and whether every load traces back through the wiring
  to a generator ("Power circuit"). That last check is the walk you
  would make along a real circuit with a meter, from the load back
  towards the supply. Most failing checks name the cable run or the
  machines involved, which is steps 3 to 5 done in one line.
- **A machine that stops may not be broken.** In the Realistic ship
  life support mode, where your home runs only on what it makes and
  stores, the electrical simulation sheds loads in priority order when
  the supply cannot carry everything: the optional ones first and the
  critical ones last. So a machine that has gone off is a symptom, and
  its cause may be upstream: not enough generation, or batteries run
  down. That is the "what still works?" question from step 2, in the
  game. (In the default Station-supplied mode aboard the ship, the
  ship's reactor makes up any shortfall, so nothing is shed.)
- **Tools wear out by use.** Each craft by hand wears every tool it
  needs by one use, and a tool breaks when its uses reach its
  durability (a hammer's base durability is 200 uses, adjusted by the
  grade it was made to). The tool rules are in
  `data/crafting/tools.ron` and the durabilities in `data/items.csv`.

What the game simplifies, so you do not learn it from the game: a tool
in the game breaks for one reason, use, and simply breaks; real tools
and machines fail for many reasons, often give warning, and many of
them can be repaired once you know why. The game does not model diagnosing a
broken tool or repairing it, and its Buildability checks are design
checks on a plan, not tests on a running machine that has developed a
fault.

## You own this when

- You make a machine safe before you touch it: every energy source
  isolated at a real isolator, stored energy released, the plug in your
  control, and a check that it is dead.
- You describe a fault in detail before you pick up a tool, including
  what still works and what changed.
- You draw the system as a chain and can say which links could and
  could not cause the symptom.
- You test at boundaries, split in the middle when you have no better
  idea, and check the cheap things first.
- You change one thing at a time, and you do not trust a new part until
  it is tested.
- After the fix, you can name the proximate cause and say whether there
  is a root cause behind it.
- You write down what you found, for the next person, who is usually
  you.
- You know where your own diagnosis stops and someone qualified takes
  over.

## Sources

Grouped by what kind of authority each one is. Every source here is a
work of the United States federal government and is therefore in the
public domain. Regulations were read in the Electronic Code of Federal
Regulations on 3 October 2026; the eCFR is updated in place.

### United States government (public domain)

- US Navy. *Navy Electricity and Electronics Training Series, Module
  19, The Technician's Handbook*, NAVEDTRA 14191, September 1998,
  Distribution Statement A, approved for public release (the six-step
  troubleshooting procedure and the hints that follow it; the safety
  observations for electrical and electronics technicians, including
  never working alone, not working on energised equipment unless
  absolutely necessary, using one hand on breakers and switches, tag-out,
  and not bypassing interlocks; capacitors retain a charge and are
  discharged before work). Copy hosted in the Internet Archive's
  NEETSModules collection. https://archive.org/details/NEETSModules
- Occupational Safety and Health Administration. 29 CFR 1910.147, The
  control of hazardous energy (lockout/tagout) (scope, including the
  exclusion of agriculture employment and the cord-and-plug exception;
  the definitions of energy source and energy isolating device, and
  that push buttons and selector switches are not isolating devices;
  relieving stored or residual energy; verifying isolation before work;
  Appendix A's sample procedure, which verifies isolation by operating
  the normal controls and then returning them to off).
  https://www.ecfr.gov/current/title-29/subtitle-B/chapter-XVII/part-1910/subpart-J/section-1910.147
- National Aeronautics and Space Administration. *NASA Procedural
  Requirements for Mishap and Close Call Reporting, Investigating, and
  Recordkeeping*, NPR 8621.1D, effective 6 July 2020, updated with
  Change 4, Appendix A, Definitions (proximate cause, intermediate cause,
  root cause, contributing factor, root cause analysis).
  https://nodis3.gsfc.nasa.gov/displayDir.cfm?Internal_ID=N_PR_8621_001D_&page_name=AppendixA
- US Consumer Product Safety Commission. *What Is a GFCI?*, CPSC Fact
  Sheet, undated (it lists requirements up to 2005) (how a GFCI works and
  the 0.006 ampere figure; ground faults most often from damaged or
  defective equipment; a receptacle GFCI protects outlets downstream on
  the branch circuit; test after installation, at least once a month
  and after a power failure; the lamp test and what to do if it fails;
  when to call a qualified electrician).
  https://www.cpsc.gov/s3fs-public/099_0.pdf
- US Environmental Protection Agency, WaterSense. Fix a Leak Week, last
  updated 13 March 2026 (the two-hour meter test; the toilet dye test
  and flushing afterwards; the common household leaks; checking gaskets,
  fittings and hose connections; worn flappers, checking them
  periodically and replacing them at least every five years; the
  cold-month baseline for a family of four).
  https://www.epa.gov/watersense/fix-leak-week

### Inside this project

- The Construction page's Buildability section
  (`src/gui/pages/construction.rs`) and the checks behind it
  (`src/machines.rs`), run over the home in `data/machines/home.ron`.
- Load shedding in the electrical simulation (`src/systems/electrical.rs`).
- Tool wear: `src/systems/crafting/tools.rs`, `data/crafting/tools.ron`
  and the `durability` column of `data/items.csv`.
- [Keeping Records](keeping_records.md), [Units and Converting
  Them](units_and_converting_them.md) and [Pests and
  Disease](pests_and_disease.md).

### Labelled in the text as general practice or our reading, not sourced

- Suspecting the most recent change first, checking cheap and easy
  things first, changing one thing at a time, putting back a change that
  did not help, checking new parts, and testing under the conditions
  that produced the fault are general practice.
- Splitting in the middle is described from its arithmetic (each test
  halves the suspects), not from a published procedure.
- Taping and labelling a breaker at home is the household version of
  OSHA's lock or tag, not a requirement.
- Reading NASA's "organizational factors" as household habits and
  schedules is our translation.
- Closing plumbing sections and repeating the meter test is our
  extension of the EPA's meter test, and unplugging everything on a
  tripping GFCI circuit and reconnecting one item at a time is general
  practice, not the CPSC's procedure.
- The torch and engine examples are general practice; the four needs of
  a petrol engine are stated without numbers, and the engine's own
  manual takes precedence.
