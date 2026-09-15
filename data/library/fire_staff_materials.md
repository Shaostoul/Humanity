# Building a Fire Staff: Every Material, and Why

A fire staff is a shaft with a fuel-soaked wick at each end. Three
subsystems decide whether it is a good one: what the shaft is made of,
what the wick is made of, and what keeps the heat out of your hands.
This guide covers all three, plus the two parts most beginner advice
leaves out entirely.

One hard boundary before anything else: **do not light a fire staff
without a trained spotter holding a fire blanket, and do not learn fire
spinning from a document.** Learn the movement with an unlit staff until
it is boring, then learn fire safety in person from experienced spinners.
Burns, hair fires, and setting bystanders alight are the real risks, and
none of them are material-selection problems. This guide makes your
equipment sound. It cannot make you safe.

Where a material number matters, it comes from a government or
manufacturer publication that measured it, not from a shop selling the
part. That distinction matters here more than usual, because most
fire-spinning technical writing is published by retailers. Every number
below traces to something in the Sources list at the bottom. Where a
number is a community convention rather than a measured value, it is
labelled **community figure** in the text, and you should treat it as
what experienced spinners tend to find rather than as a fact anyone
tested.

## Four things you will be told that are not quite right

The common advice is close but wrong in four specific places.

**"Wood, 7075-T6 aluminium, carbon fibre and titanium are the only
options."** Not the only ones, and titanium is largely a myth. The two
genuinely common shafts are 6061-T6 and 7075-T6 aluminium. Carbon fibre
is common on contact staffs. Steel, hardwood, bamboo, fibreglass and
plain steel conduit are all real and all used. A titanium staff shaft is
close to nonexistent in the trade, and where you see the word on a
product page it is usually an anodising colour or a search keyword.

**"7075-T6 is the aluminium to use."** It has the best strength and the
worst heat behaviour. NASA's materials data handbooks put 7075-T6 at
83.0 ksi ultimate tensile strength against 45.0 ksi for 6061-T6, so a
little over 1.8 times the strength, which lets you run a thinner wall.
But it starts to melt lower. The same two handbooks give a melting range
of 477 to 638 C for 7075 and 582 to 649 C for 6061, so the alloy that is
stronger cold gives up more than a hundred degrees of margin hot. Pick
7075 for stiffness at low weight, 6061 for budget and forgiveness.

**"The wick is Kevlar."** It is almost always a blend. Standard wick
tape is aramid, usually Kevlar plus Nomex, with fibreglass. Thickness
matters more than brand. It is also worth knowing that DuPont, who make
both fibres, recommend long-term service temperatures far below the
temperatures at which those fibres decompose. More on that below.

**"The parts are the shaft, the grip and the wick."** Two parts are
missing: a **heat shield or thermal break** between wick and shaft, and
the **fastening hardware** holding the wick on. Both are safety parts.
The hardware is the only place where failure means a lit wick leaves the
staff.

## The one fact that explains every other choice

Nothing you would willingly build a staff from survives the temperatures
inside a wick flame. So the whole craft is distance, shielding and
sacrifice.

| Material | Starts to fail | How it fails | Where the number comes from |
|---|---|---|---|
| Epoxy matrix (carbon fibre binder) | Softens well below any metal here | Softens, then the fibres carry nothing | Resin-specific. Get it from your tube's own data sheet; there is no single figure for "epoxy" |
| Aramid, recommended service limit | 149 to 177 C for Kevlar, 204 C for Nomex | Gradual strength loss, not sudden failure | DuPont Kevlar and Nomex technical guides |
| Silicone tape and tubing | 260 C | Degrades | Vendor rating, not an independent measurement |
| Nomex, half strength | 254 C | Breaking strength roughly halved | DuPont Nomex technical guide |
| Nomex, visible damage | 350 C | Scorches or chars in as little as 30 seconds in air | DuPont Nomex technical guide |
| Nomex, rapid breakdown | 427 C | Rapid weight loss | DuPont Nomex technical guide |
| Kevlar and other para-aramids | 427 to 482 C | Decomposes in air. Never melts | DuPont Kevlar technical guide |
| 7075-T6 aluminium | 477 C | Solidus, starts to melt | NASA materials data handbook, alloy 7075 |
| 6061-T6 aluminium | 582 C | Solidus, starts to melt | NASA materials data handbook, alloy 6061 |
| Ti-6Al-4V titanium | 1670 C (1943 K) | Melts | NIST, measured by pulse heating |

A flame does not have one temperature, and the honest range is wide.
NIST measured a 30 cm methanol pool fire and found peak mean
temperatures of about 1300 K, roughly 1030 C, near the burner. Methanol
is the cool case, because it makes no soot. Published work on sooty
candle flames takes 2000 K, about 1730 C, as a typical diffusion flame
temperature. A fire staff wick is a diffusion flame of that family, so
expect the hot regions to sit somewhere between those two figures.

Every row in the table above except titanium falls inside that band.
That is the whole point. There is no material on the list that is simply
immune, and titanium is immune only in the sense that it would survive,
not in the sense that anyone sells you a staff made of it.

The shaft under the wick never reaches those temperatures, because
evaporating fuel cools the wick and the metal conducts heat away. That
is why aluminium works at all. It is also why **a staff left burning on
the ground, with no fuel left to evaporate, is the thing that actually
destroys equipment.**

### Fuel choice does not change this table

You will see confident numbers quoted for the flame temperature of each
fuel, usually adiabatic flame temperatures around 2000 C. Two problems
with reasoning from those. First, the adiabatic flame temperature is a
theoretical ceiling: it assumes perfect mixing and no heat lost to the
surroundings, and an open wick fire in moving air never comes close to
it. Second, the tables of those values that circulate online are not
reliable. One dataset cited by an earlier version of this guide lists
naphtha at 4591 C, which is not physically possible for a hydrocarbon
burning in air, so its kerosene and gasoline figures cannot be trusted
either. That claim has been removed from this guide rather than
corrected, because there was nothing sound underneath it.

So do not pick a fuel to protect your materials. Every practical fuel
puts the wick somewhere in the band above, and every material in your
staff is already inside that band.

What genuinely differs between fuels is **radiant heat and soot**.
Candle-flame research is explicit that a flame's visible brightness is
thermal radiation emitted by incandescent soot particles. Kerosene has a
lower hydrogen-to-carbon ratio and more aromatics than camp fuel, so it
soots heavily, and more soot means more radiation reaching your skin and
your prop. Camp fuel burns cleaner and bluer and radiates less. Alcohol
soots least of all, which is exactly why its flame is so hard to see.

Burn durations are the other real difference, and here the numbers in
circulation are **community figures**: roughly 3 to 4 minutes for camp
fuel against 5 to 7 for kerosene on a comparable wick. Nobody has
published measurements for fire props, so treat those as what spinners
report, not as measured values.

Pick fuel for burn time, soot and depot safety, never to protect your
materials.

## The shaft

Nine options that genuinely get used. Thermal conductivity is in watts
per metre-kelvin at room temperature; it tells you how fast heat travels
from the wick to your hands. The two aluminium figures are from the NASA
handbooks for those specific alloys, and they are lower than the figure
you will see quoted for pure aluminium, which the Forest Products
Laboratory gives as 216. Alloying costs conductivity.

| Material | Cond. | Strengths | Weaknesses |
|---|---|---|---|
| **6061-T6 aluminium** | 167 | Cheap, stocked everywhere, bends instead of shattering, best heat spreading of the practical options | 45.0 ksi against 7075's 83.0, so a thicker wall and the heaviest metal build; flexy in thin sections |
| **7075-T6 aluminium** | 130 | A little over 1.8 times the tensile strength of 6061, so thin walls; the market default for quality staffs | Solidus 477 C, the lowest shaft metal here; poor in heat-affected zones; hard to source in thin wall |
| **Carbon fibre** | Low across the laminate, high along the fibres | Stiffest option, no flex, moves mass to the ends, survives impacts that would bend metal | The epoxy softens long before any metal here, so it *requires* aluminium sleeves; fails by shattering; never for anything you put in your mouth |
| **Titanium** | Lowest of the metals here | The only material that actually solves heat reaching your hands, and the only one here that melts above the flame at 1670 C | Expensive, poor thin-wall availability, denser than aluminium, effectively absent from the market |
| **Stainless steel** | Low, between titanium and mild steel | No temper to lose, corrosion proof, cheap next to titanium | About three times the density of aluminium; a full-length stainless staff swings badly |
| **Mild steel** | About 45 | Cheapest metal, indestructible, tolerates heat far better than aluminium | Heavy, rusts, unpleasant for fast spinning |
| **Hardwood dowel** | 0.1 to 0.2 | Over a thousand times less conductive than aluminium, so the grip stays cool; under five dollars; extra mass slows rotation, which genuinely helps a beginner; ash and hickory last decades | It is fuel; needs metal flashing at the ends; heaviest option; splits |
| **Bamboo** | Same order as hardwood | Very light for its stiffness, traditional, cool to hold, often free | Once cracked it is finished; inconsistent walls; still combustible |
| **Fibreglass** | Low, like other composites | More impact tolerant than carbon fibre, cheaper, low conductivity | Same resin problem, so it still needs a sleeve; degrades under sustained heat; rarely sold for props |

The wood figures come from the Forest Products Laboratory Wood Handbook,
which gives 0.1 to 0.14 W/(m K) for structural softwood at 12 percent
moisture and publishes the equation that puts dense hardwoods such as
ash and hickory at roughly 0.16 to 0.19. The stainless and composite
rows are ordered by physics rather than by a number, because the single
figures in circulation for those vary enormously with alloy, layup and
fibre direction and none of the sources worth citing give one value.

**Why a wooden shaft is not as mad as it sounds, and where it stops.**
The same Wood Handbook reports that wood surfaces measure somewhere
between 300 and 400 C just before piloted ignition, and that charring
becomes the dominant process above about 300 C internally. A wooden
shaft is not close to those temperatures at the grip. It is very close
to them at the wick, which is exactly why the metal flashing at the ends
is not decoration.

**The one to avoid outright: galvanised electrical conduit.** It is the
classic cheap DIY shaft and it is strong enough. The problem is the zinc
coating. Heating it releases zinc oxide fume, and the NIOSH Pocket Guide
lists the result plainly: chills, fever, muscle ache, nausea, dry throat,
metallic taste, headache, chest tightness and difficulty breathing. That
is metal fume fever. NIOSH sets the exposure limit at 5 mg per cubic
metre and considers 500 mg per cubic metre immediately dangerous to life
and health. The wick zone is exactly where you do not want zinc. Buy
plain or black steel tube instead; it costs barely more.

**If you want one recommendation:** build your first staff on a 1 inch
hardwood dowel with aluminium flashing at the ends, and spin it until
you know whether you like long or short, heavy or light. Then buy 3/4
inch 7075-T6 for the real one. The dowel costs less than the shipping on
the aluminium and answers questions no table can.

## The wick

"Kevlar" is a brand that became a category name. What you buy is a
blend, and the choices that matter are grade, thickness, and
construction, in that order.

| Grade | Actually made of | Best for |
|---|---|---|
| **K1** | Aramid blend (Kevlar and Nomex) with fibreglass | Staffs and most props. Easy to work, knots willingly, good absorption, cheap |
| **K2** | The same with more fibreglass | Eating torches. Thicker and spongier, absorbs more fuel, but frays and holds shape poorly |
| **Pure aramid** | 100 percent aramid, no glass | Hard-use staffs. Very durable, minimal edge fray, but rigid, hard on hands, awkward to knot |
| **Technora** | Copolymer aramid, rope only | Leashes, not wicks. Good heat threshold, but **poor fuel absorption**, which is disqualifying |
| **Fibreglass alone** | E-glass | Blend component only. High heat tolerance, but disintegrates from abrasion |
| **Cotton or rope** | Natural fibre | Never. Consumed in one burn, then the fastening lets go while lit |

**What the fibre makers actually say.** DuPont's Kevlar technical guide
gives a decomposition temperature in air of 427 to 482 C and is clear
that the fibre never melts. But the same guide recommends a maximum of
only 149 to 177 C for long-term use in air, and says increasing
temperature reduces tensile strength, modulus and break elongation. The
Nomex guide is similar: no melting point at all on a calorimeter trace,
but a recommended maximum continuous operating temperature of 204 C,
roughly half breaking strength at 254 C, and scorching or charring in as
little as 30 seconds at 350 C in air.

Read those two paragraphs together and you get the real picture. The
decomposition temperatures are the dramatic numbers, but the service
temperatures are the ones that explain why wicks wear out. Your wick
spends every burn far above what either manufacturer recommends for
sustained use. It is a consumable, by design, and no purchase decision
changes that.

One more thing from the Kevlar guide that matters on a fire prop:
Kevlar is flame resistant but **can be ignited**, with a limiting oxygen
index of 29. Burning usually stops once the ignition source is removed,
and, unlike nylon or polyester, it does not drip. That last property is
the reason aramid is the right fibre here and a melting synthetic is
never an option.

**Fuel does not attack the fibre.** The Kevlar guide's chemical
resistance table lists gasoline at full concentration, 21 C, for 1000
hours, with no effect on breaking strength. Whatever kills your wick, it
is heat and abrasion, not the fuel soaking in it. Hold that thought for
the grip section, where fuel very much is the problem.

**How a wick dies:** the aramid sheath chars away first, exposing the
white fibreglass core. Once that happens the wick has lost its abrasion
armour and degrades much faster from ground strikes. **White showing
through yellow is your replacement signal.** It is also why practising
over grass rather than pavement extends wick life. Both of those are
**community figures** in the sense that no one has published a study,
but the charring mechanism behind them is exactly what DuPont describes.

**Thickness is the decision that matters most.** Tape comes in 1/16,
1/8 and 1/4 inch, in widths from 1/2 to 4 inches. Suppliers are explicit
that props built from 1/4 inch material last longer, and they recommend
it specifically for staffs that get dropped. A fire staff gets dropped.
Buy 1/4 inch. For width, 3 to 5 inches is the normal staff range
(**community figure**); wider means more fuel, bigger flame, longer
burn, and more end weight.

### Construction

Burn times in this table are **community figures**, reported by makers
and spinners rather than measured under any standard.

| Type | How it is made | Burn | Trade-off |
|---|---|---|---|
| **Barrel** (flat, tape) | Tape 3 to 5 inches wide rolled around the shaft end, held with screws, rivets or aramid thread | Modest | Cheap, trivial to build, easy to rebuild with basic tools. The right first choice |
| **Monkey fist** | Aramid rope wound in three stages around a core | 7 to 9 min | Durable, aerodynamic, stalls easily, adds useful contact weight. Expensive to rebuild |
| **Crown sinnet** | Repeated crown knots up the end | Above average | Bigger, brighter flame and better looks. Higher upkeep |
| **Globe knot** | Many-faceted knot | Longest | More crossover points hold more fuel in the same volume. Biggest, roundest flame |

Barrels win on cost and maintenance; knots win on burn time, weighting
and looks. Knotted wicks also slow a staff's roll rate, which contact
spinners either want or do not.

## Heat shielding and thermal breaks

The subsystem beginner advice omits. It decides whether the staff
survives its tenth burn and whether you get burned setting it down.

Every temperature in this table is a **vendor rating**. These are
manufacturer claims about their own products, not independent
measurements, and they are usually continuous-service figures that say
nothing about brief contact with flame. Treat them as upper bounds you
should stay well under.

| Option | Vendor rating | For |
|---|---|---|
| **Aluminium sleeve** | To the alloy's solidus, 477 or 582 C | Mandatory on carbon fibre and fibreglass shafts. Convention is a sleeve about **twice the length of the wick** it shields |
| **Silicone self-fusing tape** | 260 C | The standard covering for exposed metal between wick and grip. Bonds to itself, no adhesive, no residue. The sacrificial layer you replace instead of the staff |
| **Fibreglass fire sleeve** | 260 C continuous, brief flame contact higher | Silicone-coated braided glass, sold by the foot as industrial hose protection. The closest thing to genuinely fireproof sleeving. A poor hand grip, excellent hidden layer |
| **Aramid sleeve** | Around 450 C, close to the decomposition figure rather than the service figure | Woven tube slipped over the shaft, sold as burn protection. See the wick section on why the service figure is the honest one |
| **Metal flashing** | To the metal's melting point | The traditional fix for a wooden staff: wrap the last few inches so the wood cannot catch |
| **Air gap or standoff** | Free | The cheapest thermal break there is. Mount the wick on a short stainless collar rather than directly on the shaft and the aluminium never enters the hottest zone. Under-used in DIY builds |

Shielding buys margin, not immunity. Wick covers between burns do more
for equipment life than any material upgrade.

## The grip

Grippiness is the obvious axis. For a fire staff three others matter as
much: behaviour when wet, whether fuel attacks it, and heat. The
descriptions below are practical experience rather than measured
properties, so read the whole table as a **community figure**.

| Material | Feel | Strengths | Weaknesses | Fire |
|---|---|---|---|---|
| **Silicone tubing** | Tacky, smooth | Grippiest common option when clean and dry; very heat resistant; lasts years; cleans with alcohol | Pulls hair; loses grip when humid | Yes |
| **Silicone self-fusing tape** | Slides in hand | Vendor-rated 260 C, self-bonding, forgiving, comfortable | Less outright grip than tubing | Yes, and required nearby |
| **EPDM rubber** | Padded, soft | Cheapest and lightest; holds grip better than silicone when damp; grippier after break-in | Slippery when new; leaves black residue | Suitable |
| **Goat grip** | Rough, textured | More durable than EPDM; works across most conditions | Aggressive texture, rough on skin | Suitable |
| **F-grip** | Moderately tacky | Balanced, works with some moisture, no break-in, no residue | Less dry grip than silicone; shorter life than EPDM | Suitable |
| **Tennis or hockey tape** | Thin | Cheapest, skin safe, every sports shop | Thin, no padding, wears fast; **fuel dissolves the adhesive** | Not recommended |
| **3M micro-grip** | Soft micro-texture | Unusual soft-yet-grippy surface | Least durable, most expensive, overgrip only | Overgrip only |
| **Natural foam rubber** | Very grippy | Among the grippiest available | Breaks down quickly; degrades under heat and UV | Avoid |
| **Bare or knurled metal** | Hard, cool | Nothing to wear out, nothing for fuel to attack | No padding; slippery when fuelled; conducts heat to your hands | Heat conduction |

**Is there fireproof grip tape?** No, and you do not need one. Nothing
you would want to hold survives direct flame. The nearest thing is
silicone self-fusing tape at a vendor-rated 260 C. The real answer is
geometric: the grip belongs at the centre of the staff where flame never
reaches, with silicone tape between grip and wick as the sacrificial
barrier.

**The fuel problem nobody mentions.** Grip choice is partly a
solvent-compatibility question, and it is the mirror image of the wick.
Aramid shrugs fuel off, as DuPont's thousand-hour gasoline test shows.
Adhesives do not. Naphtha and lamp oil attack the adhesives in cloth and
sports tapes, so a taped grip goes gummy, traps fuel, and becomes a wick
in the wrong place. Silicone and industrial rubber have no adhesive to
attack and shed fuel instead of absorbing it. That is the strongest
argument for silicone on a fire prop, ahead of the heat rating.

## The hardware

Small parts, disproportionate consequences.

| Method | Notes |
|---|---|
| **Stainless screws** | The mainstream answer. Two pre-drilled holes per head, screws clamping the wick tight. Add washers so the head cannot pull through the weave. Stainless keeps its strength hot |
| **Brass screws** | Traditional, chosen because brass does not rust. Common spec is a 1/8 inch self-drilling pan head. Softer, so heads strip if over-torqued |
| **Rivets** | Clean and permanent, used on commercial barrels. Not serviceable; rebuilding means drilling them out |
| **Aramid thread** | Stitching instead of piercing with metal. The standard field repair for a fraying barrel and the cheapest rebuild path. Must be genuine aramid, not polyester |
| **Stainless hose clamps** | Available for 1/2 to 7/8 inch tubing, so they fit 3/4 inch shafts. No drilling, adjustable, easy wick swaps. Bulkier, and the screw can gall when hot |
| **Safety wire** | Stainless wire as backup retention. Cheap redundancy on the one failure mode that matters |
| **End caps** | Plug the tube so fuel cannot get inside the shaft, where it cannot be reached or spun off. Often skipped. Do not skip it |

Do not use aluminium fasteners in the wick zone. Both common alloys give
up strength as they heat and begin melting at 477 and 582 C, well inside
the range the collar sees. Steel, stainless or brass only.

## Fuel

Flash point is the number to read. OSHA defines it as the minimum
temperature at which a liquid gives off enough vapour to form an
ignitable mixture with air near the liquid's surface. Flame temperature
is not a useful column, as explained above.

Flash points vary by grade and by brand, sometimes a lot, so the ranges
below are the published ranges for the substance class rather than a
figure for the bottle in your hand. The bottle's own safety data sheet is
the only thing that tells you what you actually bought.

| Fuel | Flash point | Burn | Notes |
|---|---|---|---|
| **Camp fuel** (white gas, naphtha) | Depends heavily on grade. NIOSH gives -40 to -66 C for light petroleum naphtha and -7 to 13 C for the heavier VM&P grade | 3 to 4 min (community figure) | Lights instantly, brightest and cleanest, visible in daylight, least residue, least radiant heat. CAMEO warns that vapours may travel to a source of ignition and flash back. Never for blowing |
| **Lamp oil** (paraffin) | A refined kerosene, so within the kerosene range below, and formulated by most brands toward or above its top end | 5+ min (community figure) | Slow to light, long steady burn, moderate soot, does not evaporate readily. Much safer depot behaviour. This is the one fuel here whose flash point genuinely depends on the brand, so read the bottle |
| **Kerosene** | 35 to 72 C. CAMEO gives 95 to 145 F, NIOSH gives 100 to 162 F | 5 to 7 min (community figure) | Longest burn and the most smell, smoke and residue. Heavy sooting means the most radiant heat, so it feels hottest and loads the prop most |
| **Denatured alcohol** | About 11 to 13 C. NIOSH gives 52 F for methanol and 55 F for ethanol | Short | Weak bluish flame that is nearly invisible in daylight, which is its real hazard. Low radiant output, not a low flame temperature. Contains methanol, which NIOSH lists as causing optic nerve damage and blindness, absorbed through skin as well as swallowed |
| **Petrol, diesel** | n/a | n/a | Never. No benefit, substantial added danger |

**What the classification actually says.** OSHA's flammable liquids
standard defines a flammable liquid as anything with a flashpoint at or
below 93 C, which means every fuel in that table is one, including
kerosene. Within that, Categories 1 and 2 are liquids flashing below
23 C, Category 3 covers 23 to 60 C, and Category 4 covers 60 to 93 C.

An earlier version of this guide put the dangerous boundary at "about
51 C" and said only two of these fuels were inside it. That figure does
not appear in the regulation and the framing was wrong: they are all
flammable liquids. **The split that actually matters to you is whether a
fuel gives off ignitable vapour at ordinary room temperature.** Camp
fuel and denatured alcohol do, and NIOSH classes both as Class IB
flammable liquids. Kerosene does not at typical room temperature, and
NIOSH classes it as a Class II combustible liquid.

That is a difference in how the container behaves, not in how the lit
wick behaves. For the two fuels that flash below room temperature, an
open container near any ignition source is a hazard on its own, and
CAMEO notes that their vapours can travel to a distant ignition source
and flash back to the container. Whatever you burn, **spin off the
excess before lighting**; a dripping staff is the main cause of burns.

**One toxicity note that is not about fire at all.** Both NIOSH and
ATSDR are clear that the serious poisoning risk from kerosene and
naphtha is aspiration: swallowing them and then breathing them in while
vomiting, which causes chemical pneumonitis and, in ATSDR's fatal cases,
lipoid pneumonia. This is why fuel never goes in an unlabelled drink
bottle, and why inducing vomiting is the wrong response. Methanol is a
separate problem: it blinds, and it is absorbed through the skin.

## The safety kit is part of the build

Budget for this at the same time as the staff. A first fire staff
without these is not finished.

- **A human safety spotter.** The single most important item, and not a
  material. They hold the blanket, watch you, and keep the crowd back.
  Never light up alone.
- **A fire blanket.** Duvetyne, a heavy fire-retardant cotton, is the
  community standard; a wet towel is the accepted substitute. Both of
  those are **community figures**, not tested recommendations. Note the
  failure mode: a blanket soaked in fuel will itself catch, so keep it
  dry and let it air between uses.
- **Natural fibre clothing.** The US Forest Service tested this
  properly. Their wildland fire policy permits only undergarments of 100
  percent natural fibre, meaning cotton, wool or silk, or aramid, and
  prohibits polyester, polypropylene and nylon because they may melt and
  aggravate burn injuries. In the Forest Service's own flame-engulfment
  tests, the polypropylene and synthetic-blend shirts melted, and the
  report notes that melting material may stick to skin. Worth being
  precise about the mechanism: the measured heat transferred through
  synthetic undershirts was only slightly worse than cotton, so the
  danger is not that synthetics let more heat through. It is that they
  melt onto you. Aim for 100 percent natural fibre, and no synthetic
  layer against the skin. Nothing dangly, whatever the fibre.
- **Hair tied back, no oil-based product in it.**
- **A fuel depot away from the performance area.** A sealed metal
  container and a spin-off station, both away from any ignition source,
  and one person responsible for it. Remember the flashback warning: the
  ignition source does not have to be near the container.
- **A fire extinguisher, CO2 or ABC dry powder, at the fuel depot.**
  CAMEO's response guidance for petroleum naphtha is foam, carbon
  dioxide or dry chemical, and states plainly that water may be
  ineffective; for kerosene it warns that water spray may be inefficient
  and that solid streams should never be aimed directly at the product.
  A blanket handles a wick or a person; it does not handle a spilled and
  ignited depot, which is the hazard the depot itself creates. If you
  burn alcohol, note that its flame can be effectively invisible in
  daylight, so you may need an indirect way to tell whether something is
  still alight.
- **Burn first aid.** Cool running water first, and plenty of it.
  MedlinePlus says to run cool water over a burn, not ice water, and to
  keep it under water for at least 5 to 30 minutes. Burn-care guidelines
  summarised by CADTH call for at least 20 minutes of cool running water
  and state that ice or ice water should not be used, because it can
  cause hypothermia and impair blood flow to the injured tissue. Do not
  put butter, oil, cream or any household remedy on a serious burn.
  Know where the nearest hospital is before you light up.
- **A wick cover** for between burns. Extinguishes reliably, keeps soot
  off everything, and extends wick life.
- **Green or damp ground, not pavement and not dry grass.** Damp grass,
  dirt or gravel. Never dry grass, leaf litter, scrub, heath or forest
  floor: a dripping staff plus dry ground is how a wildfire starts, and
  dripping is the failure mode this guide warns about twice. Check local
  fire restrictions and that you have permission to be there at all.

## Sizing

Every figure in this table is a **community figure**, taken from what
production staffs are built to and what spinners settle on. None of it
is a measured or specified value.

| Dimension | Typical | Why |
|---|---|---|
| Shaft diameter | 3/4 inch (19 mm) | The US standard for contact staffs, and the size most hardware is built around |
| Wall thickness | 0.058 inch, or 1.5 to 2 mm composite | Thicker raises weight and moment of inertia, so slower and less responsive. Never below 1.5 mm on a composite tube |
| Overall length | 55 to 59 inches (140 to 150 cm) | Where commercial contact staffs cluster. Traditional fitting is chin to nose height |
| Wick width | 3 to 5 inches tape | Sets flame size and burn time. Cut the shaft about 1 inch longer than the wick width per head |
| Heat sleeve | 2x wick length | The convention for composite shafts |
| Finished weight | 700 to 850 g | A production 7075 contact staff at 3/4 inch and 55 to 59 inches |
| Burn time | 7 to 9 min | What that same staff delivers per fuelling |

## Build order

This is genuinely sequential. Out of order means re-buying parts.

1. **Decide length and style before material.** Contact and rolling work
   wants stiffer, longer, end-weighted. Fast spinning wants shorter and
   lighter. This drives diameter, wall and wick construction.
2. **Pick the shaft.** Order it cut to length plus about 1 inch per head
   for the wick seat. If it is composite, order the aluminium sleeves in
   the same purchase; the staff is unusable without them.
3. **Fit the thermal break and sleeves first.** Retrofitting a sleeve
   past a mounted wick means taking the wick off again.
4. **Mount the wick.** Pre-drill, then clamp tight with stainless or
   brass screws and washers. Tight matters: a loose barrel unrolls as
   the tape relaxes with heat.
5. **Silicone tape the transition zone**, from just below the wick down
   toward the grip. This is the layer that stops you grabbing hot metal.
6. **Cap the ends** so fuel cannot get inside the shaft.
7. **Grip last, and mark the centre** in a contrasting colour. Every
   production staff marks it, and it matters more than it sounds.
8. **Check balance dry, then burn it in** with a spotter and a blanket.
   Watch for the wick loosening as the tape relaxes, and for how hot the
   shaft gets at your hands. Retighten afterwards; almost every new
   barrel wick needs it.

## What it costs

Material totals for one double-ended staff, excluding tools. Prices are
United States, 2026, and move constantly; check before ordering. These
are retail survey figures, not data, and they age faster than anything
else in this guide.

**A correction to an earlier version of this table.** It priced the wick
column at "$76 (100 ft roll)". That is the cheapest of the supplier's
twenty-odd variants, 1/16 inch thick and half an inch wide, which is the
opposite of what this guide recommends two sections earlier: 1/4 inch
thick at 3 to 5 inches wide. Thicker and wider tape costs several times
more, so the old total understated the proper build substantially. Price
the exact thickness and width you intend to buy, by the foot, before
committing. Be aware too that thickness and width are not independent in
the catalogue: the thickest tape is not stocked in the widest sizes, so
"1/4 inch at 4 inches wide" may not be purchasable from one supplier at
all, and you may have to trade one recommendation against the other.

| Component | Learner (hardwood) | First build (6061) | Proper staff (7075) |
|---|---|---|---|
| Shaft | $5 | $55 | $90 to $200 |
| Wick | $14 to $32 | $14 to $32 | see note below |
| Heat shielding | $8 | $12 to $20 | $25 to $45 |
| Grip | $6 | $12 to $20 | $20 to $35 |
| Hardware | $4 | $8 | $15 |
| Safety kit | $25 | $50 | $70 |
| Fuel | $15 | $15 | $25 |
| **Total** | **$77 to $94** | **$166 to $200** | **$225 to $390 plus wick** |

**The build-versus-buy number.** A finished production contact fire
staff in 3/4 inch 7075 with knotted wicks sells for around $185.
Equivalent materials cost more than that, because raw tube and a full
wick roll are sold in quantities far larger than one staff needs.

So building is not the cheap route for a single staff. It is the cheap
route for the second one, and the only route to something that is
exactly what you want. If price is the only concern, buy the production
staff and put the difference into fuel and practice.

## You own this when

- You can name what your shaft is made of and what temperature it starts
  to fail at.
- Your wick is 1/4 inch material, mounted with steel or brass, and you
  know that white showing through yellow means replace it.
- There is a deliberate thermal break between wick and grip, and you can
  say what it is made of.
- You picked your fuel for burn time and depot safety, not because
  someone told you it burns colder.
- You know whether your fuel gives off ignitable vapour at room
  temperature, because that decides how the container has to be handled.
- You have a spotter, a dry blanket, natural fibres, and grass
  underfoot, every single time.

When all six are true you own a sound staff and you understand why each
part is what it is. That understanding is the part that transfers:
matching a material to a temperature, isolating heat, and knowing which
failure mode actually hurts someone are the same skills behind every
piece of equipment you will ever build or repair.

## Sources

Grouped by what kind of authority each one is. Government publications
are listed first because they are public domain and can be redistributed
with this guide; the rest cannot.

### United States government (public domain)

- Occupational Safety and Health Administration. 29 CFR 1910.106,
  Flammable liquids (definitions of flammable liquid, the four
  categories, and flashpoint).
  https://www.govinfo.gov/content/pkg/CFR-2024-title29-vol5/xml/CFR-2024-title29-vol5-sec1910-106.xml
- National Institute for Occupational Safety and Health. NIOSH Pocket
  Guide to Chemical Hazards: Kerosene.
  https://www.cdc.gov/niosh/npg/npgd0366.html
- NIOSH Pocket Guide to Chemical Hazards: Petroleum distillates
  (naphtha). https://www.cdc.gov/niosh/npg/npgd0492.html
- NIOSH Pocket Guide to Chemical Hazards: VM & P Naphtha.
  https://www.cdc.gov/niosh/npg/npgd0664.html
- NIOSH Pocket Guide to Chemical Hazards: Methyl alcohol.
  https://www.cdc.gov/niosh/npg/npgd0397.html
- NIOSH Pocket Guide to Chemical Hazards: Ethyl alcohol.
  https://www.cdc.gov/niosh/npg/npgd0262.html
- NIOSH Pocket Guide to Chemical Hazards: Zinc oxide (fume).
  https://www.cdc.gov/niosh/npg/npgd0675.html
- National Oceanic and Atmospheric Administration. CAMEO Chemicals:
  Kerosene. https://cameochemicals.noaa.gov/chemical/960
- CAMEO Chemicals: Petroleum naphtha, V.M. & P.
  https://cameochemicals.noaa.gov/report?key=CH12319
- CAMEO Chemicals: Methanol.
  https://cameochemicals.noaa.gov/chemical/3874
- Agency for Toxic Substances and Disease Registry. Toxicological
  Profile for Fuel Oils, Health Effects chapter (kerosene aspiration and
  dermal effects). https://www.ncbi.nlm.nih.gov/books/NBK594684/
- NASA. Muraca, R.F. and Whittick, J.S., Materials Data Handbook:
  Aluminum Alloy 6061, NASA-CR-123772, 1972 (6061-T6 conductivity,
  strength and melting range).
  https://ntrs.nasa.gov/api/citations/19720022808/downloads/19720022808.pdf
- NASA. Muraca, R.F. and Whittick, J.S., Materials Data Handbook:
  Aluminum Alloy 7075, NASA-CR-123773, 1972 (7075-T6 conductivity,
  strength and melting range).
  https://ntrs.nasa.gov/api/citations/19720022809/downloads/19720022809.pdf
- National Institute of Standards and Technology. Thermophysical
  Measurements on 90Ti-6Al-4V Alloy Above 1450 K Using a Transient
  (Subsecond) Technique, Journal of Research of the NBS, Vol. 81A
  (titanium alloy melting point, 1943 K).
  https://nvlpubs.nist.gov/nistpubs/jres/81A/jresv81An2-3p251_A1b.pdf
- USDA Forest Products Laboratory. Wood Handbook, FPL-GTR-113,
  Chapter 3, Physical Properties and Moisture Relations of Wood (wood
  thermal conductivity, the conductivity equation, and the comparison
  values for aluminium and steel).
  https://www.fpl.fs.usda.gov/documnts/fplgtr/fplgtr113/ch03.pdf
- USDA Forest Products Laboratory. Wood Handbook, FPL-GTR-113,
  Chapter 17, Fire Safety (wood surface temperature before piloted
  ignition, charring behaviour).
  https://www.fpl.fs.usda.gov/documnts/fplgtr/fplgtr113/ch17.pdf
- USDA Forest Service, Missoula Technology and Development Center.
  Petrilli, T. and Ackerman, M., Tests of Undergarments Exposed to Fire
  (natural versus synthetic fibre, melting and burn injury).
  https://www.fs.usda.gov/t-d/pubs/pdfpubs/pdf08512348/pdf08512348dpi72.pdf
- National Library of Medicine. MedlinePlus, Burns (first aid, cooling
  duration, what not to put on a burn).
  https://medlineplus.gov/ency/article/000030.htm

### Peer-reviewed literature

- Wang, Z., Tam, W.C., Chen, J., Lee, K.Y. and Hamins, A., Thin Filament
  Pyrometry Field Measurements in a Medium-Scale Pool Fire, Fire
  Technology, 2019 (NIST authors; measured pool fire temperatures).
  https://pmc.ncbi.nlm.nih.gov/articles/PMC7593899/
- Verdugo, I., Cruz, J.J., Alvarez, E., Reszka, P., Figueira da Silva,
  L.F. and Fuentes, A., Candle flame soot sizing by planar time-resolved
  laser-induced incandescence, Scientific Reports 10, 11364, 2020 (soot
  as the source of flame luminosity; typical diffusion flame
  temperature). https://pmc.ncbi.nlm.nih.gov/articles/PMC7347618/

### Health technology assessment

- Canadian Agency for Drugs and Technologies in Health. Cooling for
  Thermal Burns: Clinical Effectiveness and Guidelines, 2019 (cooling
  duration; why not to use ice).
  https://www.ncbi.nlm.nih.gov/books/NBK541209/

### Manufacturer technical literature (named product, not "an SDS")

- DuPont. Technical Guide for Kevlar Aramid Fiber (decomposition
  temperature, recommended long-term service temperature, limiting
  oxygen index, chemical resistance to gasoline).
  https://www.r-g.de/wiki/images/e/ec/Td_en_Kevlar_guide.pdf
- DuPont. Technical Guide for NOMEX Brand Fiber (thermogravimetric
  behaviour, absence of a melting point, maximum continuous operating
  temperature, strength retention with temperature).
  https://www.nakedwhiz.com/gasketsafety/nomextechnicalguide.pdf

### Removed from this guide

These were cited by earlier versions and are no longer used. They are
listed so nobody re-adds them.

- Every retailer and hobbyist blog previously cited for material
  properties, including dark-monk.com, bonoboflow.com, firemecca.com,
  homeofpoi.com, fireandflow.co.nz, flowartsinstitute.com,
  drexfactor.com, kloecknermetals.com, sendcutsend.com and
  hontitan.com. Marketing copy is not an authority on melting points,
  and every number they were cited for is now sourced above or removed.
- hypertextbook.com, a student-compiled physics figure collection, which
  was the basis for the flame temperature claims.
- zenodo.org/records/7236629, a self-published dataset of adiabatic
  flame temperatures. It lists naphtha at 4591 C, which is not
  physically possible, so nothing in it is safe to quote.

### Not authorities, but useful for one practical detail

- davlyngroup.com, a manufacturer of silicone-coated fibreglass fire
  sleeving. Vendor page. Useful only as a description of what that
  product is and what the maker claims for it; its temperature ratings
  are vendor claims and are labelled as such in the text above.
