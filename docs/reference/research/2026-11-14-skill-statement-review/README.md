# Skill statement review (2026-11-14)

Every skill has a ladder of named stages (Novice to Master), and each stage says what a person at it
can do in plain "I can" statements (`data/skills/stages.json` holds the keys, `data/i18n/en.json` the
words; the design is `docs/design/skills.md`, section 3). These statements teach real skills, so on
2026-11-14, before skills rung 1 merged, all 810 of them were reviewed adversarially against primary
sources, the way teaching data in this project is checked like code. This folder records what the
review found and what was done with each finding.

- `findings.json`: the record. Every finding with its key, stage, severity, problem, the statement as
  it was, the reviewer's reasoning (`why`), the reviewer's correction, its source (url, the source's
  own date, a quotation) and its outcome: the changes made, or the reason for rejecting it.
- `crates/humanity-systems/src/skills/statement_review_tests.rs` holds the data to the record: every
  finding applied (words changed, moved to the stage it named, or a new statement added) or rejected
  with a reason, and every corrected statement carrying its source in `stages.json`'s `sources`.

## Method

1. **Refutation.** Three reviewers each took a share of the families and tried to refute every
   statement: unsafe as written, missing a legal condition, in the wrong stage (a step that must come
   before something an earlier stage already does), factually wrong, or too vague to check. A finding
   was filed only with a primary source: its url, its own date, and a quotation of the words that
   carry the meaning. Every site's robots.txt was read first, in a step of its own; a site that refused
   automated reading was not worked around and no re-hosted copy was read (the refusals are listed
   below). Two order findings cite the project's own stage model, because the error was in the ladder.
2. **Judging and applying.** Each finding was judged on its source and its reasoning. An applied
   finding rewords the statement, moves it to the stage it belongs in (under that stage's number and
   the next unused letter), or adds the missing step as a new statement. Where the reviewer's words
   were changed, the record's `note` says why: reordered so the statement starts "I can" like every
   other; a legal condition stated generally ("where the law allows", "the licence the law
   requires") when the source was one country's; a two-part statement split in two; a stage emptied
   by a move given a statement of its own.
3. **Recording.** The sources of every corrected or added statement are in `stages.json` (`sources`:
   key, url, date); the quotations are below and in `findings.json`.

## Counts

| Group | Families | Statements checked | Findings |
|---|---|---|---|
| A | Body and health, Safety and rescue, Combat | 198 | 30 |
| B | Food and water, Science | 153 | 37 |
| C | Materials and making, Building and engineering, Moving and travel, Community, Flow and prop arts, Arts | 459 | 36 |
| All | | 810 | 103 |

| Severity | Findings | Applied | Rejected |
|---|---|---|---|
| high | 13 | 13 | 0 |
| medium | 60 | 60 | 0 |
| low | 30 | 30 | 0 |
| all | 103 | 103 | 0 |

By problem: factual 8, legal 20, order 13, safety 50, vague 12.

Applied as the reviewer wrote it: 82. Applied with the words adjusted (each with a note below): 21. Rejected: 0.

No finding was rejected: each one's source supports it, and where a reviewer's wording went
further than its source (a country's rule written as everyone's), the wording was generalised
rather than the finding refused. The weakest is `skills.medicine.s4_a`: its legal claim (that
diagnosing others is the licensed practice of medicine) has no statute quotation, since every state
legislature site tried refused or failed. It is applied on its safety reasoning alone, and the new
words make no legal claim.

## New statements and moves

| Key | Stage | Kind | From finding | Words |
|---|---|---|---|---|
| `skills.swimming.s2_c` | beginner | moved from `skills.swimming.s4_b` | `skills.swimming.s4_b` | I can call the emergency number and help someone in trouble in the water by shouting to them, reaching with a pole or rope, or throwing something that floats, without ever going into the water myself. |
| `skills.climbing.s3_c` | competent | added | `skills.climbing.s4_a` | I can belay a partner on a top rope, having learned on a course, with one hand always on the brake side of the rope. |
| `skills.radiation_safety.s4_c` | proficient | added | `skills.radiation_safety.s4_b` | I can explain that solar storms are a radiation danger to astronauts and high-flying aircrew, not to people on the ground, and in a radiation emergency I get inside, stay inside for at least 24 hours and follow official instructions. |
| `skills.gunnery.s1_b` | novice | added | `skills.gunnery.s1_a` | I can explain that real gunnery is learned only in the armed forces or in a simulator, and that mounted and automatic weapons are banned or tightly restricted for private people in most countries. |
| `skills.preserving.s3_c` | competent | moved from `skills.preserving.s4_a` | `skills.preserving.s4_a` | I can explain why low-acid food (meat, fish, poultry, milk and fresh vegetables other than most tomatoes) must be pressure canned and never canned in boiling water. |
| `skills.preserving.s4_c` | proficient | added | `skills.preserving.s4_a` | I can pressure can low-acid food with a tested recipe, adjusting for my altitude, in a canner whose gauge has been checked. |
| `skills.hunting.s3_c` | competent | moved from `skills.hunting.s4_a` | `skills.hunting.s4_a` | I can choose a shot that gives a quick, humane kill, and follow up carefully if an animal is hurt. |
| `skills.hunting.s4_c` | proficient | moved from `skills.hunting.s5_a` | `skills.hunting.s5_a` | I can field dress game cleanly wearing gloves, keep the meat cool and clean, and have deer or elk tested for chronic wasting disease where the local wildlife agency advises it. |
| `skills.hunting.s5_b` | expert | added | `skills.hunting.s5_a` | I can plan and carry out a whole hunt, from scouting to bringing the meat home, safely, within the law and with no prompts. |
| `skills.fishing.s2_c` | beginner | added | `skills.fishing.s4_a` | I can measure a fish and return one I may not keep, alive and quickly. |
| `skills.geology.s1_b` | novice | added | `skills.geology.s4_b` | I can keep out of old mines and tunnels and away from cliff edges and the foot of cut faces. |
| `skills.weather.s1_b` | novice | added | `skills.weather.s4_b` | I can go into a substantial building or a hard-topped vehicle as soon as I hear thunder, never shelter under a tree, and stay there until 30 minutes after the last thunder. |
| `skills.construction.s2_c` | beginner | added | `skills.construction.s4_b` | I can set up and climb a ladder safely and tell when scaffold or edge protection is needed. |
| `skills.flying.s2_c` | beginner | added | `skills.flying.s4_a` | I can handle a simulated engine failure and other emergencies with an instructor, and land safely. |
| `skills.fire_performance.s3_c` | competent | added | `skills.fire_performance.s3_b` | I can fuel a prop safely, dipping the wick, spinning off the excess fuel into a spin-off bucket well away from the performance area, keeping the fuel capped and away from flame, and never using petrol. |
| `skills.mediation.s2_c` | beginner | moved from `skills.mediation.s3_b` | `skills.mediation.s3_b` | I can spot when a disagreement involves threats, violence or abuse, or is too serious for me, and get help instead of trying to settle it myself. |

Other edits made with this pass: the six baking statements gained the full stop every other
statement has (the review noted it as style), and the Expert example in `docs/design/skills.md`,
section 3, follows `skills.baking.s5_a`'s new words.

## Every finding

Each entry: the statement as it was, what it says now (or why it was rejected), and the source.

### Group A: Body and health, Safety and rescue, Combat

#### `skills.first_aid.s3_a` (competent, high, safety)

- Was: "I can check whether a person is responding and breathing, and place them in the recovery position."
- Now: "I can check whether a person is responding and breathing normally. If they are unresponsive but breathing normally, I put them in the recovery position and keep checking their breathing; if they are not breathing normally (gasps do not count), I call the emergency number and start CPR."
- Source: https://www.sja.org.uk/get-advice/first-aid-advice/recovery-position/, clinically reviewed 2025-04-28: "It’s safe to place someone in the recovery position who is not responding to you but is breathing normally."

#### `skills.first_aid.s3_b` (competent, medium, vague)

- Was: "I can do chest compressions and rescue breaths on a practice dummy at the right pace."
- Now: "I can give chest compressions on a practice manikin about 5 to 6 cm deep at 100 to 120 a minute, with 30 compressions to 2 rescue breaths, send someone for a defibrillator, and follow a training defibrillator's spoken instructions."
- Source: https://www.sja.org.uk/first-aid-advice/cpr/, clinically reviewed 2026-09-18: "Push at a rate of 100 to 120 per minute."

#### `skills.first_aid.s2_b` (beginner, medium, safety)

- Was: "I can cool a burn with clean running water and cover it loosely."
- Now: "I can cool a burn under cool running water for at least 20 minutes (never ice, creams or butter), take off rings or clothing near it before it swells unless they are stuck, and then cover it loosely with cling film or a clean plastic bag."
- Source: https://www.sja.org.uk/first-aid-advice/burns-and-scalds/, clinically reviewed 2025-04-28: "Hold it under cool running water for at least 20 minutes or until the pain feels better."

#### `skills.first_aid.s2_a` (beginner, low, safety)

- Was: "I can press firmly on a bleeding wound with a clean cloth and keep the pressure on."
- Now: "I can put on gloves if I have them, get someone to call the emergency number for heavy bleeding, and press firmly on the wound with a clean pad, keeping the pressure on."
- Source: https://www.sja.org.uk/first-aid-advice/severe-bleeding/, clinically reviewed 2026-09-08: "With open wounds there's a risk of infection, so wear protective first aid gloves (if available)"

#### `skills.first_aid.s4_b` (proficient, low, vague)

- Was: "I can spot shock, heat illness and cold injury and begin the right care while help comes."
- Now: "I can spot shock, heat exhaustion and heatstroke, and cold injuries such as hypothermia and frostbite, and give the first steps for each while help comes; for heatstroke I call the emergency number and start cooling the person at once with a cool, wet sheet or cold water."
- Source: https://www.sja.org.uk/first-aid-advice/heatstroke/, clinically reviewed 2025-04-28: "Call 999 or 112 for emergency help."

#### `skills.swimming.s1_a` (novice, medium, safety)

- Was: "I can enter shallow water with a helper nearby, float on my back and breathe out into the water."
- Now: "I can enter shallow water with a helper who can swim watching me closely the whole time and staying within reach, float on my back and breathe out into the water."
- Source: https://www.cdc.gov/drowning/prevention/index.html, 2025-09-29: "Designate a responsible adult to supervise closely and constantly when children are in or near water (including bathtubs)."

#### `skills.swimming.s4_b` (proficient, medium, order)

- Was: "I can help someone in trouble using a rope, pole or floating object while staying on dry ground."
- Moved to beginner as `skills.swimming.s2_c`: "I can call the emergency number and help someone in trouble in the water by shouting to them, reaching with a pole or rope, or throwing something that floats, without ever going into the water myself."
- Source: https://www.cheshirefire.gov.uk/your-safety/water-safety/what-to-do-in-a-water-emergency/, last updated 2026-05-27: "Never enter the water to attempt a rescue; instead, use the advice below and call 999 as soon as possible."

#### `skills.climbing.s1_a` (novice, low, factual)

- Was: "I can climb a fixed ladder with three points of contact while a helper steadies the base."
- Now: "I can climb a short portable ladder set on firm, level ground, facing it and keeping three points of contact, while a helper steadies the base."
- Source: https://www.osha.gov/sites/default/files/publications/portable_ladder_qc.pdf, OSHA 3246 Portable Ladder Safety QuickCard (file dated 2020-09-28): "Always maintain a 3-point (two hands and a foot, or two feet and a hand) contact on the ladder when climbing."

#### `skills.climbing.s4_a` (proficient, medium, order)

- Was: "I can climb a route at my level on a rope with a belayer, calling clearly to my partner."
- Added at competent as `skills.climbing.s3_c`: "I can belay a partner on a top rope, having learned on a course, with one hand always on the brake side of the rope."
- Note: The partner check in the reviewer's words is left out because Competent's s3_b already says it (check my partner's knot and harness, and be checked by them).
- Source: https://www.alpenverein.de/files/kletterhallen/indoor-klettern-basics-english.pdf, DAV "Indoor climbing basics" (English), file dated 2025-04-29: "One hand always holding on to the brake side of the rope."

#### `skills.medicine.s4_a` (proficient, medium, safety)

- Was: "I can compare the possible causes of a set of symptoms and say what to watch for next."
- Now: "I can describe a person's symptoms clearly and in order to a health worker, check a trusted health service's list of warning signs, and know which signs mean get help now, leaving the diagnosis to a qualified professional."
- Source: https://www.cdc.gov/heads-up/response/index.html, 2025-09-15: "the decision about return to sports is a medical decision that should be made by a healthcare provider."

#### `skills.sanitation.s4_b` (proficient, medium, safety)

- Was: "I can trace where a water or waste system is failing and isolate or fix the problem."
- Now: "I can trace where a water or waste system is failing from the outside and isolate or fix simple faults, without ever climbing or leaning into a tank, pit or manhole (its gases can kill in minutes), and I call a qualified plumber or the water authority for anything more."
- Source: https://www.osha.gov/laws-regs/regulations/standardnumber/1910/1910.146, 29 CFR 1910.146, current text (no date shown): "Has limited or restricted means for entry or exit (for example, tanks, vessels, silos, storage bins, hoppers, vaults, and pits are spaces that may have limited means of entry.)"

#### `skills.sanitation.s1_a` (novice, low, vague)

- Was: "I can wash my hands properly with soap and clean water at the moments that matter."
- Now: "I can wash my hands with soap and clean running water, scrubbing the backs, between the fingers and under the nails for at least 20 seconds, before handling food and after the toilet, touching animals or waste, or caring for someone who is sick."
- Source: https://www.cdc.gov/clean-hands/about/index.html, 2024-02-16: "Scrub your hands for at least 20 seconds."

#### `skills.sanitation.s2_b` (beginner, low, safety)

- Was: "I can clean, then disinfect, a surface touched by raw meat or a sick person."
- Now: "I can clean, then disinfect, a surface touched by raw meat or a sick person, using one disinfectant as its label says and never mixing bleach with ammonia or any other cleaner."
- Source: https://www.cdc.gov/natural-disasters/safety/index.html, 2026-08-26: "Never mix bleach with ammonia or any other cleaner."

#### `skills.sanitation.s3_b` (competent, low, vague)

- Was: "I can set up hand washing and waste disposal for a camp or small group."
- Now: "I can set up hand washing and waste disposal for a camp or small group, with latrines or catholes 6 to 8 inches (15 to 20 cm) deep and at least 200 feet (about 60 m) from water, camp and trails, and washing water carried the same distance from streams."
- Source: https://www.nps.gov/articles/leave-no-trace-seven-principles.htm, last updated 2022-06-16: "deposit solid human waste in catholes dug 6 to 8 inches deep, at least 200 feet from water, camp and trails."

#### `skills.fire_making.s1_a` (novice, medium, legal)

- Was: "I can clear a safe spot, keep water and a shovel near, and light a fire with a lighter or match under an adult's eye."
- Now: "I can check that fires are allowed here today, use an existing fire ring or clear a circle at least 10 feet (3 m) across and at least 15 feet (4.5 m) from tents, plants and low branches, keep water and a shovel near, and light a fire with a lighter or match while an experienced person watches."
- Source: https://smokeybear.com/en/prevention-how-tos/campfire-safety, no page date (footer: 2026 Ad Council): "Don’t build a campfire or dig a fire pit if the campground, area, event rules, or local laws prohibit them."

#### `skills.fire_making.s2_b` (beginner, low, vague)

- Was: "I can put a fire fully out until the ashes are cold to the touch."
- Now: "I can put a fire fully out by drowning it with water until the hissing stops, stirring the ashes and drowning them again, then holding the back of my hand over them to check that no heat is left."
- Source: https://smokeybear.com/en/prevention-how-tos/campfire-safety, no page date (footer: 2026 Ad Council): "Hover the back of your hand over the ashes to feel that the embers are cool"

#### `skills.fire_making.s4_b` (proficient, low, safety)

- Was: "I can burn wood into charcoal for the kiln with a steady method."
- Now: "I can burn wood into charcoal outdoors, well away from buildings and never in an enclosed space (it gives off carbon monoxide), using a pit, drum or kiln method I can describe step by step."
- Source: https://www.cdc.gov/carbon-monoxide/about/index.html, 2026-10-26: "Never burn charcoal indoors."

#### `skills.firefighting.s3_a` (competent, low, vague)

- Was: "I can put out a small fire with a blanket or the correct extinguisher."
- Now: "I can put out a fire with a fire blanket or the correct extinguisher (pull, aim low, squeeze, sweep), only when others have been alerted, the fire service has been called, the fire is small and contained to one object such as a pan or a bin, I am clear of the smoke and I have an escape route behind me."
- Source: https://www.usfa.fema.gov/prevention/home-fires/prepare-for-fire/fire-extinguishers/, last reviewed 2023-04-01: "Is the fire small and contained in a single object or to a surface (like a pan or a wastebasket)?"

#### `skills.radiation_safety.s4_b` (proficient, medium, factual)

- Was: "I can decide when to shelter, and when to leave, during a solar storm."
- Now: "I can decide when to go to the ship's storm shelter, and when to leave it, during a solar storm."
- Added at proficient as `skills.radiation_safety.s4_c`: "I can explain that solar storms are a radiation danger to astronauts and high-flying aircrew, not to people on the ground, and in a radiation emergency I get inside, stay inside for at least 24 hours and follow official instructions."
- Note: The reviewer's one statement with "In the game:" and "In real life:" halves is split into two statements, each starting "I can" like every other.
- Source: https://www.gov.uk/guidance/space-weather-and-radiation, published 2014-01-01: "far too small to produce an observable health effect."

#### `skills.rescue.s3_a` (competent, medium, safety)

- Was: "I can free a person from simple dangers, such as a fallen branch or a stuck door, without being hurt myself."
- Now: "I can free a person from simple dangers, such as a fallen branch or a stuck door, after checking that no power line is down or touching it, and without being hurt myself; if a line is down, I keep everyone away and call the emergency number."
- Source: https://www.osha.gov/sites/default/files/publications/downed_electrical_wires.pdf, OSHA fact sheet FS-3941, 02/2018: "Circuits do not always turn off when a power line falls into a tree or onto the ground."

#### `skills.tracking.s1_a` (novice, low, vague)

- Was: "I can keep a safe distance from wildlife and never feed or touch wild animals."
- Now: "I can stay at least 100 yards (91 m) from bears, wolves and big cats and at least 25 yards (23 m) from other large animals, or further where local rules say so, and never feed or touch wild animals."
- Source: https://www.nps.gov/yell/planyourvisit/safety.htm, last updated 2026-07-28: "Always stay at least 100 yards (91 m) away from bears, wolves, and cougars"

#### `skills.tracking.s2_a` (beginner, low, safety)

- Was: "I can find and describe footprints, droppings and feeding signs."
- Now: "I can find and describe footprints, droppings and feeding signs without touching droppings with bare hands, and I wash my hands afterwards."
- Source: https://www.cdc.gov/baylisascaris/about/index.html, 2024-11-05: "Most infections are in children and others who are more likely to put dirt or animal waste in their mouth by mistake."

#### `skills.unarmed.s3_b` (competent, low, safety)

- Was: "I can spar lightly with control, defending as often as I attack, and stop at once when my partner taps or calls it."
- Now: "I can spar lightly with control under a coach's eye, defending as often as I attack, stop at once when my partner taps or calls it, and stop for the day and get checked if either of us takes a hard knock to the head or feels dizzy or confused."
- Source: https://www.cdc.gov/heads-up/response/index.html, 2025-09-15: "Remove the athlete from sports participation right away if a concussion is suspected."

#### `skills.impact_weapons.s4_b` (proficient, low, safety)

- Was: "I can use a tool such as an axe or hammer for its real job, such as splitting wood, with control and a clear work area."
- Now: "I can use an axe or hammer for its real job, such as splitting wood, wearing safety glasses and stout boots, with secure footing, feet about shoulder width apart, my spare hand clear of the blade and nobody near me."
- Source: https://www.fs.usda.gov/t-d/pubs/htmlpubs/htm99232823/page04.htm, USDA Forest Service MTDC 9923-2823P (1999; no date on the page): "Always wear safety glasses or goggles for eye protection."

#### `skills.archery.s2_b` (beginner, low, safety)

- Was: "I can fit and look after my gear, such as the bowstring and arrows, and spot damage before it becomes dangerous."
- Now: "I can fit and look after my gear, such as the bowstring and arrows, spot damage before it becomes dangerous, and never dry-fire a bow (draw and release it without an arrow)."
- Source: https://myodfw.com/learn/skill/bow-shooting-tips, no page date (copyright 2026): "Never dry-fire a bow – drawing and releasing the string without a nocked arrow."

#### `skills.archery.s4_b` (proficient, low, vague)

- Was: "I can shoot from other positions, such as kneeling, and at moving or unmarked targets with a safe backstop."
- Now: "I can shoot from other positions, such as kneeling, and at moving targets or targets at distances I have to judge myself, always sure of my target and of what is in front of and behind it, with a safe backstop."
- Source: https://myodfw.com/learn/skill/bow-shooting-tips, no page date (copyright 2026): "Take the time to make sure of your target before you shoot, and take note of what’s in front of and behind your target."

#### `skills.marksmanship.s1_a` (novice, medium, safety)

- Was: "I can treat every firearm as loaded, keep the muzzle in a safe direction, keep my finger off the trigger and know my backstop."
- Now: "I can treat every firearm as loaded, keep the muzzle pointed in a safe direction, keep my finger off the trigger until I am ready to shoot, and be certain of my target and what is beyond it, and I handle firearms only with a qualified instructor."
- Source: https://idfg.idaho.gov/press/field-hunting-safety-reminders, 2013-11-05: "Be certain of your target and what is beyond it."

#### `skills.marksmanship.s2_a` (beginner, medium, legal)

- Was: "I can check that a firearm is unloaded, load and unload it safely and hold it with a steady stance and grip."
- Now: "I can check that a firearm is unloaded, load and unload it safely, and hold it with a steady stance and grip, under a qualified instructor and where the law allows."
- Note: Same content, reordered so the statement starts "I can"; "where the law allows" is the general legal wording the review brief asks for.
- Source: https://www.legislation.gov.uk/ukpga/1968/27/section/1, Firearms Act 1968 s.1, revised text up to date to 2026-11-15: "to have in his possession, or to purchase or acquire, a firearm to which this section applies without holding a firearm certificate in force at the time"

#### `skills.marksmanship.s3_b` (competent, medium, safety)

- Was: "I can safely deal with a simple stoppage the way I was taught, clean and store the firearm, and follow range commands without the guide."
- Now: "I can safely deal with a simple stoppage the way I was taught, keeping the muzzle pointed safely, clean the firearm, and store it unloaded and locked away from anyone not allowed to use it, with the ammunition stored separately as the law where I live requires, and I follow range commands without the guide."
- Source: https://www.gov.uk/government/publications/firearms-security-handbook/firearms-security-handbook-2020-accessible, Home Office Firearms security handbook 2020, updated 2021-01-20: "It is a condition of the issue of a firearm certificate that the firearms and shotguns to which the certificate relates must be stored securely at all times so as to prevent, so far as is reasonably practicable, access to the guns by unauthorised persons."

#### `skills.gunnery.s1_a` (novice, medium, legal)

- Was: "I can follow the safety steps for a mounted weapon: check it is safe, clear the firing zone and call out before firing."
- Added at novice as `skills.gunnery.s1_b`: "I can explain that real gunnery is learned only in the armed forces or in a simulator, and that mounted and automatic weapons are banned or tightly restricted for private people in most countries."
- Note: The game statement s1_a is kept as the reviewer asked; the real-life note becomes an "I can explain" statement beside it.
- Source: https://www.law.cornell.edu/uscode/text/18/922, 18 U.S.C. 922(o), current text (no date shown): "Except as provided in paragraph (2), it shall be unlawful for any person to transfer or possess a machinegun."

### Group B: Food and water, Science

#### `skills.preserving.s2_b` (beginner, high, factual)

- Was: "I can spot spoiled food by its smell, look or swollen container and throw it away without tasting."
- Now: "I can spot the warning signs of spoiled food (a bulging, leaking or spurting container, mould, a bad smell) and throw it out without tasting it, and I know food can carry botulism toxin with no sign at all, so I trust only food preserved by a tested method."
- Source: https://www.cdc.gov/botulism/prevention/home-canned-foods.html, 2024-04-25 (date shown on page): "You cannot see, smell, or taste the toxin that causes botulism."

#### `skills.preserving.s4_a` (proficient, high, order)

- Was: "I can explain why low-acid food such as vegetables and meat must be pressure canned, and why boiling water is not enough."
- Moved to competent as `skills.preserving.s3_c`: "I can explain why low-acid food (meat, fish, poultry, milk and fresh vegetables other than most tomatoes) must be pressure canned and never canned in boiling water."
- Added at proficient as `skills.preserving.s4_c`: "I can pressure can low-acid food with a tested recipe, adjusting for my altitude, in a canner whose gauge has been checked."
- Source: https://www.cdc.gov/botulism/prevention/home-canned-foods.html, 2024-04-25 (date shown on page): "Pressure canning is the only recommended method for canning low-acid foods."

#### `skills.preserving.s3_a` (competent, medium, safety)

- Was: "I can make jam or pickles in clean, sealed jars using a tested recipe."
- Now: "I can make jam or pickles with a tested recipe and process the filled jars in a boiling-water canner for the time it gives, adjusted for my altitude."
- Source: https://nchfp.uga.edu/blog/why-do-you-recommend-processing-jams-and-jellies, 2018-05-02: "recommend a boiling water canning process for jams and jellies"

#### `skills.preserving.s2_a` (beginner, medium, order)

- Was: "I can salt, dry or cool-store food and say why each method keeps it safe."
- Now: "I can dry or cool-store food and say why each method keeps it safe."
- Note: The reviewer's bracketed pointer (salting waits for Proficient's tested curing method, s4_b) is a note for the ladder, not words for the statement; it is kept here instead.
- Source: https://www.cdc.gov/mmwr/volumes/68/wr/mm6844a3.htm, 2019-11-08: "traditional methods of home preparation for these dishes might support conditions that are favorable for toxin production"

#### `skills.fermenting.s4_a` (proficient, high, factual)

- Was: "I can make beer or wine safely with sanitised equipment and know why the sealed vessel matters."
- Now: "I can make beer or wine with sanitised equipment, using an airlock that lets gas out and keeps air out, and I bottle only when fermentation has finished, because trapped gas can burst a fermenter or a bottle."
- Source: https://hgic.clemson.edu/factsheet/basics-of-home-winemaking/, 2023-02-24: "allow the carbon dioxide produced during fermentation to escape"

#### `skills.fermenting.s2_a` (beginner, medium, safety)

- Was: "I can tell a healthy ferment from a bad one by its smell, look and taste."
- Now: "I can tell a healthy ferment from a bad one by its smell and look before anyone tastes it, and I throw out one that is slimy, smells rotten or grows fuzzy or coloured mould."
- Source: https://www.cdc.gov/botulism/prevention/home-canned-foods.html, 2024-04-25 (date shown on page): "Never taste food to see if it's safe."

#### `skills.fermenting.s3_a` (competent, low, safety)

- Was: "I can make yoghurt, vinegar or kombucha without the guide."
- Now: "I can make yoghurt, vinegar or kombucha without the guide, and I know homemade vinegar's strength is unknown, so I never use it to pickle or can food for storage."
- Source: https://nchfp.uga.edu/how/pickle/general-information-pickling/general-information-on-pickling/, undated (read 2026-11-14): "Do not ... use a vinegar with unknown acidity."

#### `skills.cooking.s2_a` (beginner, medium, safety)

- Was: "I can chop, boil, fry and roast common foods and tell when they are properly cooked through."
- Now: "I can chop, boil, fry and roast common foods, and check that meat, poultry and egg dishes are cooked through with a food thermometer, not by their colour."
- Source: https://www.cdc.gov/food-safety/prevention/index.html, 2025-11-24: "The only way to tell if food is safely cooked is to use a food thermometer."

#### `skills.cooking.s2_b` (beginner, medium, safety)

- Was: "I can keep raw and cooked food apart and handle hot pans and knives safely."
- Now: "I can keep raw and cooked food apart, handle hot pans and knives safely, and put out a small pan fire by sliding a lid over it and turning off the heat, never with water."
- Source: https://www.usfa.fema.gov/prevention/home-fires/prevent-fires/cooking/, 2026-09-28 (page last reviewed): "Use it to cover the pan if it catches on fire. This will put out the fire."

#### `skills.cooking.s4_b` (proficient, medium, safety)

- Was: "I can change a recipe for an allergy, a diet or a missing ingredient."
- Now: "I can change a recipe for an allergy, a diet or a missing ingredient, reading every label for the allergen and keeping the meal free of it from the board to the plate."
- Source: https://www.fda.gov/food/nutrition-food-labeling-and-critical-foods/food-allergies, 2026-03-11 (content current as of): "severe, life-threatening symptoms, often called anaphylaxis, that may involve fatal respiratory problems and shock"

#### `skills.nutrition.s3_b` (competent, low, safety)

- Was: "I can adjust a meal for allergies or a common diet such as vegetarian."
- Now: "I can adjust a meal for allergies, checking every label and keeping out cross-contact, or for a common diet such as vegetarian."
- Source: https://www.fda.gov/food/nutrition-food-labeling-and-critical-foods/food-allergies, 2026-03-11 (content current as of): "the inadvertent introduction of a major food allergen into a product"

#### `skills.nutrition.s4_a` (proficient, low, safety)

- Was: "I can plan meals for children, older people or hard physical work."
- Now: "I can plan meals for children, older people or hard physical work, and I know babies have their own rules (no honey before 12 months) and follow a health professional's advice for them."
- Source: https://www.cdc.gov/infant-toddler-nutrition/foods-and-drinks/foods-and-drinks-to-avoid-or-limit.html, 2026-04-14: "Do not give your child honey before 12 months."

#### `skills.water_purification.s2_b` (beginner, medium, factual)

- Was: "I can bring water to a full rolling boil to make it safe to drink."
- Now: "I can bring water to a rolling boil for one minute (three minutes above about 2,000 m) to kill germs, and I know boiling does not remove chemicals or fuel."
- Source: https://www.cdc.gov/water-emergency/about/index.html, 2024-09-19: "You cannot make water containing fuel, toxic chemicals, or radioactive materials safe by boiling or disinfecting it."

#### `skills.water_purification.s3_a` (competent, medium, safety)

- Was: "I can use a filter or purification tablets and follow the instructions exactly."
- Now: "I can use a filter or purification tablets correctly and say what each one misses (most filters let viruses through; chlorine and iodine are weak against parasites such as Cryptosporidium), and combine a filter with a disinfectant when the water needs both."
- Source: https://www.cdc.gov/healthywater/drinking/travel/backcountry_water_treatment.html, 2025-01-30: "Chlorine bleach and iodine kill bacteria and viruses but do not work well to kill parasites."

#### `skills.hunting.s1_a` (novice, medium, safety)

- Was: "I can show the basic rules of handling a firearm, treating every one as loaded, under a qualified supervisor."
- Now: "I can show the four firearm safety rules under a qualified supervisor: treat every firearm as loaded, keep the muzzle pointed in a safe direction, keep my finger off the trigger until I am ready to shoot, and be sure of my target and what is beyond it."
- Note: Same content, reordered so the statement starts "I can".
- Source: https://idfg.idaho.gov/press/field-hunting-safety-reminders, 2013-11-05: "Always control the muzzle of your firearm."

#### `skills.hunting.s2_a` (beginner, medium, legal)

- Was: "I can find out and follow the local rules for where, when and what I may hunt."
- Now: "I can complete any hunter education course and get the licence and tags the law requires where I hunt, and follow the local rules for where, when and what I may hunt, including any rule on wearing bright orange."
- Note: Generalised: the source is one US state's (Idaho), so "the hunter education course" and "hunter-orange rule" become "any hunter education course" and "any rule on wearing bright orange", per the review brief's rule on legal wording.
- Source: https://idfg.idaho.gov/hunt/education, undated (read 2026-11-14): "Hunter Ed Certification is required for all hunters born on or after January 1, 1975"

#### `skills.hunting.s4_a` (proficient, medium, order)

- Was: "I can choose the shot that gives a quick, humane kill, and follow up carefully if an animal is hurt."
- Moved to competent as `skills.hunting.s3_c`: "I can choose a shot that gives a quick, humane kill, and follow up carefully if an animal is hurt."
- Source: docs/design/skills.md, section 3, Stages, not numbers, 2026-11-14: "Every lower statement stays required."

#### `skills.hunting.s5_a` (expert, medium, order)

- Was: "I can field dress game cleanly and keep the meat cool, clean and safe to eat."
- Moved to proficient as `skills.hunting.s4_c`: "I can field dress game cleanly wearing gloves, keep the meat cool and clean, and have deer or elk tested for chronic wasting disease where the local wildlife agency advises it."
- Added at expert as `skills.hunting.s5_b`: "I can plan and carry out a whole hunt, from scouting to bringing the meat home, safely, within the law and with no prompts."
  - Its source: docs/design/skills.md (2026-11-14): "| 5 Expert | Fast, excellent work with no prompts |"
- Note: "where my state advises it" is US wording; generalised to "where the local wildlife agency advises it". Moving s5_a down left Expert with no statement (every stage needs one), so s5_b is new: the Expert stage's own meaning (docs/design/skills.md section 3) applied to the whole hunt.
- Source: https://www.cdc.gov/chronic-wasting/about/index.html, 2026-01-21: "Strongly consider having the deer or elk tested for CWD before you eat the meat."

#### `skills.fishing.s1_a` (novice, medium, safety)

- Was: "I can handle a hook with care and cast only when the space around me is clear."
- Now: "I can handle a hook with care, cast only when the space around me is clear, and wear a life jacket in a boat or near deep or fast water."
- Source: https://www.uscgboating.org/multimedia/news-detail.php?id=573, undated news item (2023 statistics; page last modified November 2026): "Of those drowning victims with reported life jacket usage, 87% were not wearing a life jacket."

#### `skills.fishing.s2_a` (beginner, medium, legal)

- Was: "I can set up a line, bait a hook and land a small fish following the steps."
- Now: "I can check that I have any licence the water needs and know its size and catch limits, then set up a line, bait a hook and land a small fish following the steps."
- Source: https://www.gov.uk/fishing-licences, undated (read 2026-11-14): "You can get a fine of up to £2,500 if you’re fishing in these areas and cannot show a valid rod fishing licence when asked."

#### `skills.fishing.s4_a` (proficient, medium, order)

- Was: "I can release a fish unharmed when it is undersized or out of season."
- Added at beginner as `skills.fishing.s2_c`: "I can measure a fish and return one I may not keep, alive and quickly."
- Now: "I can unhook and release a fish with the best chance of survival, even a deep-hooked one."
- Source: https://www.gov.uk/freshwater-rod-fishing-rules/fish-size-and-catch-limits, undated (read 2026-11-14): "You must return fish you cannot keep to the water unharmed."

#### `skills.fishing.s5_a` (expert, low, safety)

- Was: "I can fish a day to feed a group, handle the catch properly and keep it safe to eat."
- Now: "I can fish a day to feed a group, check the local fish-eating advice for that water, handle the catch properly and keep it safe to eat."
- Source: https://www.epa.gov/choose-fish-and-shellfish-wisely/should-i-be-concerned-about-eating-fish-and-shellfish, 2026-01-12: "look for fish advisories for specific fish and the waterbodies where the fish or shellfish are caught"

#### `skills.beekeeping.s1_a` (novice, medium, safety)

- Was: "I can wear protective clothing and use a smoker calmly near a hive with an experienced keeper beside me."
- Now: "I can wear protective clothing and use a smoker calmly near a hive with an experienced keeper beside me, and I know what to do if I am stung, including when a reaction is an emergency."
- Note: Same content, reordered so the statement starts "I can".
- Source: https://www.cdc.gov/niosh/outdoor-workers/about/insects-and-scorpions.html, 2026-09-28 (last reviewed): "carry an EpiPen and wear a medical identification bracelet or necklace"

#### `skills.beekeeping.s4_a` (proficient, medium, legal)

- Was: "I can spot common pests and diseases and take the right first step."
- Now: "I can spot common pests and diseases and take the right first step, including reporting a notifiable disease such as foulbrood to the authorities where the law requires it."
- Note: Generalised: "the bee inspector" is the English Order's office; "the authorities" holds wherever such a law exists.
- Source: https://www.legislation.gov.uk/uksi/2006/342/article/3, latest revised version (read 2026-11-14): "shall immediately notify that fact to the Secretary of State."

#### `skills.foraging.s2_b` (beginner, medium, legal)

- Was: "I can find a few plants I know in the right season and place without damaging the area."
- Now: "I can find a few plants I know in the right season and place, where picking is allowed and with the landowner's permission, without damaging the area."
- Source: https://www.legislation.gov.uk/ukpga/1981/69/section/13, up to date with changes in force on or before 2026-11-15: "not being an authorised person, intentionally uproots any wild plant not included in that Schedule."

#### `skills.foraging.s3_a` (competent, medium, safety)

- Was: "I can name the dangerous lookalikes of each plant I pick and show how I tell them apart."
- Now: "I can name the dangerous lookalikes of each plant or fungus I pick and show how I tell them apart (for a mushroom: its cap, gills, spore print and the whole base)."
- Source: https://www.cdc.gov/mmwr/volumes/66/wr/mm6621a1.htm, 2017-06-02: "Amatoxins, consisting of alpha, beta, and gamma amanitin, account for >90% of deaths related to mushroom poisoning worldwide."

#### `skills.foraging.s5_a` (expert, medium, safety)

- Was: "I can identify plants from poor or incomplete samples by cross-checking several features, and still refuse any doubtful one."
- Now: "I can narrow down plants and fungi from poor or incomplete samples by cross-checking several features, and I never call one edible unless I have the whole specimen, including the base of a mushroom."
- Source: https://explore.beatymuseum.ubc.ca/mushroomsup/A_phalloides.html, undated (read 2026-11-14): "if the base breaks off in the soil, its diagnostic cup can easily be missed."

#### `skills.animal_husbandry.s3_b` (competent, medium, legal)

- Was: "I can handle an animal calmly, check its feet and coat, and treat minor problems."
- Now: "I can handle an animal calmly, check its feet and coat, and treat minor problems, using medicines only as labelled or prescribed by a vet and keeping to their withdrawal times before using milk, eggs or meat."
- Source: https://www.fda.gov/animal-veterinary/antimicrobial-resistance/gfi-263-frequently-asked-questions-farmers-and-ranchers, 2024-05-22 (content current as of): "As of June 11, 2023, all new affected products entering the marketplace must have prescription labeling."

#### `skills.geology.s4_b` (proficient, high, order)

- Was: "I can judge when ground is unstable, such as a slope, an old shaft or a cut face, and keep away."
- Added at novice as `skills.geology.s1_b`: "I can keep out of old mines and tunnels and away from cliff edges and the foot of cut faces."
- Note: s4_b stays as it was, as the reviewer asked; the Novice rule is new.
- Source: https://www.msha.gov/stay-out-stay-alive, undated (read 2026-11-14): "Many vertical shafts can be hundreds of feet deep and completely unprotected"

#### `skills.geology.s4_a` (proficient, medium, legal)

- Was: "I can follow clues in soil, stream gravel and rock to trace where an ore may lie."
- Now: "I can follow clues in soil, stream gravel and rock to trace where an ore may lie, on land where I have permission to look and take samples."
- Source: https://www.nps.gov/articles/recreational-collection-of-rocks-and-minerals-legal-instruments.htm, 2024-01-02 (last updated): "prohibits possessing, destroying, disturbing mineral resources…in park units."

#### `skills.weather.s4_b` (proficient, high, safety)

- Was: "I can recognise signs of a thunderstorm or a front approaching and move to shelter."
- Added at novice as `skills.weather.s1_b`: "I can go into a substantial building or a hard-topped vehicle as soon as I hear thunder, never shelter under a tree, and stay there until 30 minutes after the last thunder."
- Now: "I can recognise the signs of a thunderstorm or a front approaching early and reach safe shelter (a substantial building or a hard-topped vehicle) before it arrives."
- Note: The Novice rule is reordered so it starts "I can".
- Source: https://www.weather.gov/safety/lightning-tips, undated (read 2026-11-14): "a substantial building with electricity or plumbing or an enclosed, metal-topped vehicle with windows up."

#### `skills.astronomy.s2_a` (beginner, low, factual)

- Was: "I can find north at night using a star pattern or a compass."
- Now: "I can find north at night from a star pattern (Polaris in the northern hemisphere; in the southern, I find south from the Southern Cross) or with a compass, knowing a compass shows magnetic north, which can differ from true north."
- Source: https://www.ngdc.noaa.gov/geomag/declination.shtml, undated (read 2026-11-14): "Magnetic declination, sometimes called magnetic variation, is the angle between magnetic north and true north."

#### `skills.butchery.s2_b` (beginner, low, safety)

- Was: "I can break down a small bird or rabbit into its main parts."
- Now: "I can break down a small bird or rabbit into its main parts, wearing gloves when it is wild game."
- Source: https://www.cdc.gov/tularemia/prevention/index.html, 2025-09-09: "Use gloves when handling animals, especially rabbits, muskrats, prairie dogs, and other rodents."

#### `skills.butchery.s2_a` (beginner, low, vague)

- Was: "I can keep raw meat cold and apart from other food, and clean up properly afterwards."
- Now: "I can keep raw meat at 4 °C (40 °F) or colder and apart from other food, and clean up properly afterwards."
- Source: https://www.fda.gov/consumers/consumer-updates/are-you-storing-food-safely, 2023-01-18 (content current as of): "Keep the refrigerator temperature at or below 40° F (4° C)."

#### `skills.farming.s5_a` (expert, low, safety)

- Was: "I can grow a wide range of crops well and judge soil by touch, smell and sight."
- Now: "I can grow a wide range of crops well, judge soil by touch, smell and sight, and know when to send a sample for a lab test, since contamination such as lead cannot be seen."
- Source: https://www.epa.gov/lead/healthy-gardening-practices-reduce-exposure-lead-contaminated-soil, 2026-09-21 (last updated): "Knowing how much lead is in your soil is a good place to start"

#### `skills.baking.s4_b` (proficient, low, safety)

- Was: "I can bake in a wood or camp oven and manage its heat."
- Now: "I can bake in a wood oven or a camp oven (a camp oven only outdoors, never in a tent or indoors) and manage its heat."
- Source: https://www.cdc.gov/carbon-monoxide/about/index.html, 2026-10-26: "Never burn charcoal indoors."

#### `skills.baking.s5_a` (expert, low, vague)

- Was: "I can run a full bake for forty people to a standard"
- Now: "I can run a full bake for forty people, on time, with every loaf evenly baked through and every allergen labelled."
- Source: docs/design/skills.md, section 3, the stage table, 2026-11-14: "Fast, excellent work with no prompts"

### Group C: Materials and making, Building and engineering, Moving and travel, Community, Flow and prop arts, Arts

#### `skills.forestry.s1_a` (novice, medium, vague)

- Was: "I can put on helmet, eye and ear protection and chaps, and keep clear while someone experienced fells a tree."
- Now: "I can put on a helmet, eye and ear protection and chaps, and stay at least two tree lengths away while someone experienced fells a tree."
- Source: https://teagasc.ie/wp-content/uploads/2025/05/Basic_Chainsaw_Felling_-_IFSG_302.pdf, October 2018 (Health and Safety Authority, Ireland, IFSG 302); read 2026-11-14: "A safe working distance of two tree lengths must be maintained, unless exceptional conditions and a comprehensive risk assessment dictate otherwise."

#### `skills.forestry.s2_b` (beginner, medium, safety)

- Was: "I can trim and cut up a small fallen tree into lengths with a hand saw, standing safely."
- Now: "I can trim and cut up a small fallen tree that lies flat and is not bent or caught, using a hand saw, standing on the uphill side, cutting the compressed side first, and leaving any springy or trapped trunk to someone trained."
- Source: https://teagasc.ie/wp-content/uploads/2025/05/Cross_Cutting_and_Manual_Stacking_-_IFSG_304.pdf, October 2018 (Health and Safety Authority, Ireland, IFSG 304); read 2026-11-14: "When cross-cutting on slopes, work on the uphill side of logs if there is a risk of the timber rolling. Reduce excessive tension by first making a cut on the compression side of the log"

#### `skills.forestry.s4_a` (proficient, high, safety)

- Was: "I can spot and release tension in a tree that is under strain, and deal with one that is caught or leaning wrongly."
- Now: "I can spot and release tension safely in a tree that is under strain, and bring down a hung-up tree with a felling lever, turning tool or winch, never by felling another tree onto it, cutting the tree that holds it, climbing it or working under it."
- Source: https://teagasc.ie/wp-content/uploads/2025/05/Basic_Chainsaw_Felling_-_IFSG_302.pdf, October 2018 (Health and Safety Authority, Ireland, IFSG 302); read 2026-11-14: "Do not: fell the supporting tree in an attempt to free the hung-up tree; climb a hung-up tree; fell another tree across the hung-up tree in an attempt to dislodge it; walk or work under a hung-up tree"

#### `skills.mining.s1_a` (novice, medium, safety)

- Was: "I can wear a helmet, eye and ear protection and a dust mask, and keep clear of loose rock while following the guide."
- Now: "I can wear a helmet, eye and ear protection and a fitted respirator rated for fine dust (FFP3 or equivalent, not a nuisance dust mask), and keep clear of loose rock while following the guide."
- Source: https://www.hse.gov.uk/agriculture/dustmasks.htm, updated 2024-08-07; read 2026-11-14: "Some retailers also sell products known as nuisance dust masks or comfort masks that look like dust respirators but are not marked with CE or with any standard."

#### `skills.mining.s3_a` (competent, medium, safety)

- Was: "I can plan a dig or quarry face so that it is stable, drains water, and has a clear way out."
- Now: "I can plan a dig or quarry face so that its sides are supported or cut back to a safe angle, it drains water and has a clear way out, and I have a competent person inspect it before anyone works in it."
- Source: https://www.hse.gov.uk/construction/safetytopics/excavations.htm, 2022-02-10 (date shown on page); read 2026-11-14: "No ground can be relied upon to stand unsupported in all circumstances."

#### `skills.metallurgy.s2_b` (beginner, low, safety)

- Was: "I can sort metals by sight and by simple tests and skim slag off a melt."
- Now: "I can sort metals by sight and by simple tests, set aside galvanised, painted or coated scrap and unknown alloys whose fumes can make me ill, and skim slag off a melt."
- Source: https://www.hse.gov.uk/welding/health-risks-welding.htm, 2026-01-09; read 2026-11-14: "Metal fume fever is usually linked to welding or hot work on galvanised metals."

#### `skills.welding.s2_b` (beginner, medium, safety)

- Was: "I can clean and clamp joints properly and avoid welding near anything that can burn."
- Now: "I can clean and clamp joints properly, move or cover anything that can burn well clear of the sparks, keep an extinguisher and someone watching for fire, and never weld or cut a drum, tank or container that has held fuel or chemicals unless someone trained has made it safe."
- Source: https://www.hse.gov.uk/pubns/indg314.pdf, INDG314(rev1), published 05/13; read 2026-11-14: "Tanks and drums that seem to be ‘empty’ usually still have residues in the bottom, in seams and other crevices. Just a teaspoon of petrol in a drum can be enough to cause an explosion"

#### `skills.machining.s1_a` (novice, medium, safety)

- Was: "I can wear eye protection, tie back hair and loose clothing, and follow the steps to make one simple cut on a lathe or mill."
- Now: "I can wear eye protection, tie back hair, take off gloves, rings and watches, keep loose clothing clear, and follow the steps to make one simple cut on a lathe or mill."
- Source: https://www.hse.gov.uk/engineering/getting-started.htm, 2025-02-24; read 2026-11-14: "Many serious accidents that occur in engineering workshops could be avoided if the wearing of gloves was prevented whilst working in close proximity to rotating machinery such as drills and lathes."

#### `skills.ceramics.s1_a` (novice, medium, safety)

- Was: "I can wedge clay and make a simple pinch pot, wearing a dust mask while handling dry clay."
- Now: "I can wedge clay and make a simple pinch pot, clean dry clay dust with water or a filtered vacuum instead of sweeping, and wear a rated respirator (FFP3 or equivalent, not a nuisance dust mask) when dust cannot be avoided."
- Source: https://www.hse.gov.uk/agriculture/dustmasks.htm, updated 2024-08-07; read 2026-11-14: "These are not intended for use when a respirator should be worn. You should never use a nuisance dust mask instead"

#### `skills.ceramics.s4_b` (proficient, high, safety)

- Was: "I can make bricks, tiles or a ceramic water filter that works."
- Now: "I can make bricks or tiles that hold up, and a ceramic water filter whose flow I test, and I still disinfect the water it makes (for example with chlorine), because a filter does not reliably remove viruses."
- Source: https://www.cdc.gov/yellow-book/hcp/preparing-international-travelers/water-disinfection-for-travelers.html, last updated April 23, 2025 (CDC Yellow Book); read 2026-11-14: "Many filters do not reliably remove viruses"

#### `skills.electronics.s4_a` (proficient, medium, safety)

- Was: "I can repair a damaged board by replacing parts and fixing broken tracks."
- Now: "I can repair a damaged board by replacing parts and fixing broken tracks, with the power off and stored energy in capacitors and batteries released first; mains power supplies and microwave ovens I leave to someone trained."
- Source: https://www.hse.gov.uk/electricity/faq.htm, last updated 2026-05-07; read 2026-11-14: "You can be reasonably sure that your electrical equipment is safe to work on if all sources of energy ... have been securely isolated and any stored energy has been released from the equipment."

#### `skills.construction.s4_b` (proficient, medium, order)

- Was: "I can work safely at height with ladders and scaffold and brace a wall while it is built."
- Added at beginner as `skills.construction.s2_c`: "I can set up and climb a ladder safely and tell when scaffold or edge protection is needed."
- Now: "I can work safely at height from scaffold and brace a wall while it is built."
- Note: The ladder half moves to Beginner as a new statement; Proficient keeps the scaffold and the bracing.
- Source: https://www.hse.gov.uk/construction/safetytopics/roofwork.htm, 2026-06-05; read 2026-11-14: "High risk: almost one in five deaths in construction work involve roof work."

#### `skills.construction.s5_a` (expert, medium, legal)

- Was: "I can lead the building of a sound house or ship section to a standard, with no prompts."
- Now: "I can lead the building of a sound house or ship section to a standard, with no prompts, getting the building approval or permits it needs and passing its inspections."
- Source: https://www.gov.uk/building-regulations-approval, dateModified 2025-01-24; read 2026-11-14: "You must check if you need approval before you construct or change buildings in certain ways."

#### `skills.masonry.s1_a` (novice, low, safety)

- Was: "I can wear eye protection, gloves and a dust mask, and mix mortar following the steps."
- Now: "I can wear eye protection, gloves and a rated respirator (FFP3 or equivalent, not a nuisance dust mask), and mix mortar following the steps."
- Source: https://www.hse.gov.uk/agriculture/dustmasks.htm, updated 2024-08-07; read 2026-11-14: "Some retailers also sell products known as nuisance dust masks or comfort masks that look like dust respirators but are not marked with CE or with any standard."

#### `skills.electrical.s2_a` (beginner, high, safety)

- Was: "I can use a tester to prove a circuit dead, and never work on a live one."
- Now: "I can prove a circuit dead with a proper two-pole voltage tester, checking the tester on a proving unit before and after the test, testing every conductor, and I never work on a live circuit."
- Source: https://www.hse.gov.uk/pubns/priced/hsg85.pdf, HSG85 third edition, published 08/2013; read 2026-11-14: "It will be necessary to test the instrument before and after use. This may be done by means of a proving unit with a low power output."

#### `skills.electrical.s3_a` (competent, high, legal)

- Was: "I can wire a simple circuit with a switch, a light and a fuse or breaker without the guide."
- Now: "I can wire a simple circuit with a switch, a light and a fuse or breaker on a training board without the guide, and before touching a real installation I follow the local rules (a permit, notifying building control, or a licensed electrician)."
- Source: https://assets.publishing.service.gov.uk/media/5a802da7ed915d74e622ceed/BR_PDF_AD_P_2013.pdf, Approved Document P, 2013 edition (England); read 2026-11-14: "A person intending to carry out building work ... is required to give a building notice or deposit full plans where the work consists of-- (a) the installation of a new circuit"

#### `skills.electrical.s4_b` (proficient, medium, safety)

- Was: "I can connect a solar panel and battery system so that it is protected."
- Now: "I can connect a small solar panel and battery system so that it is protected, covering the panels while I work because they are live in daylight; a system wired into a home I leave to a licensed installer, or work under one, where local rules require it."
- Note: The last clause is reworded to read plainly; the review's summary says the "live in daylight" wording is common practice with no quoted source, kept because it is plain physics (a lit panel makes voltage).
- Source: https://www.energy.gov/cmei/systems/guide-fire-safety-solar-systems, last updated February 21, 2025 (US Department of Energy); read 2026-11-14: "utilize properly licensed installers to ensure the safety of these systems"

#### `skills.electrical.s5_a` (expert, medium, legal)

- Was: "I can plan, install and test a whole system to a standard, safely and with no prompts."
- Now: "I can plan, install and test a whole system to the local wiring rules, with the permit, notification or licence they require, safely and with no prompts."
- Source: https://assets.publishing.service.gov.uk/media/5a802da7ed915d74e622ceed/BR_PDF_AD_P_2013.pdf, Approved Document P, 2013 edition (England); read 2026-11-14: "is required to give a building notice or deposit full plans where the work consists of-- (a) the installation of a new circuit; (b) the replacement of a consumer unit"

#### `skills.hvac.s3_a` (competent, medium, safety)

- Was: "I can size a heater or cooler for a room and set it up without the guide."
- Now: "I can size a heater or cooler for a room and set it up without the guide; a fuel-burning heater I have installed by a qualified person, vented as its maker says, with a carbon monoxide alarm fitted."
- Source: https://www.cpsc.gov/safety-education/safety-guides/carbon-monoxide/carbon-monoxide-fact-sheet, no date shown on page; read 2026-11-14: "Make sure appliances are installed and operated according to the manufacturer's instructions and local building codes. Most appliances should be installed by qualified professionals."

#### `skills.hvac.s4_b` (proficient, medium, legal)

- Was: "I can set up a heat pump or a heat recovery fan and check that it works."
- Now: "I can choose and place a heat pump or a heat recovery fan and check that it works; connecting or charging the heat pump's refrigerant circuit I leave to a certified refrigerant technician."
- Source: https://www.gov.uk/guidance/qualifications-required-to-work-on-equipment-containing-f-gas, dateModified 2026-03-03; read 2026-11-14: "For RACHP systems you must have qualifications to: install new systems; service and maintain systems; check for leaks"

#### `skills.life_support.s2_a` (beginner, medium, factual)

- Was: "I can tell the signs of too little oxygen or too much carbon dioxide and call for help quickly."
- Now: "I can read the oxygen and carbon dioxide meters and alarms, know that too little oxygen can knock me out with no warning I would notice, and leave and call for help as soon as a reading or alarm says so."
- Source: https://www.csb.gov/assets/1/6/nitrogen_asphyxiation_safety_bulletin_(6-11-03).pdf, CSB Safety Bulletin No. 2003-10-B, June 2003; read 2026-11-14: "The exposed person has no warning and cannot sense that the oxygen level is too low."

#### `skills.rigging.s3_a` (competent, medium, order)

- Was: "I can pick the right knots, pulleys and rope for a load and rig a lift without the guide."
- Now: "I can work out a load's weight, pick knots, pulleys, slings and rope whose marked safe working load covers it, and rig a simple lift without the guide."
- Note: The reviewer's bracketed pointer (the weight estimate moves here from Proficient) is a note, not words for the statement; Proficient's s4_b keeps the harder part (sling angles).
- Source: https://www.hse.gov.uk/pubns/indg290.pdf, INDG290(rev1), published 03/13; read 2026-11-14: "Don't... exceed the safe working load of machinery or accessories like chains, slings and grabs."

#### `skills.rigging.s4_b` (proficient, low, vague)

- Was: "I can work out a load's weight and choose gear with a safe margin."
- Now: "I can work out a load's weight, including the extra force that wide sling angles put on each leg, and never use gear beyond its marked safe working load; if I doubt the weight, I do not lift."
- Source: https://www.hse.gov.uk/pubns/indg290.pdf, INDG290(rev1), published 03/13; read 2026-11-14: "Remember that the load in the legs of a sling increases as the angle between the legs increases"

#### `skills.mechanics.s2_b` (beginner, high, safety)

- Was: "I can change a tyre and a filter and check brakes and belts."
- Now: "I can change a tyre and a filter and check brakes and belts, putting the vehicle on axle stands on firm, level ground whenever a wheel is off or any part of me goes under it; I never rely on a jack alone."
- Source: https://www.hse.gov.uk/mvr/mechanical-repair/under-vehicles.htm, 2025-09-19; read 2026-11-14: "vehicles are always properly supported - do not rely on jacks alone"

#### `skills.repair.s1_a` (novice, medium, safety)

- Was: "I can unplug a broken item, look it over, and follow the steps to fix a simple fault."
- Now: "I can unplug a broken item, look it over, and follow the steps to fix a simple fault, and I know that some items, such as microwave ovens and power supplies, hold a dangerous charge after unplugging, so I leave those to someone trained."
- Source: https://www.hse.gov.uk/electricity/faq.htm, last updated 2026-05-07; read 2026-11-14: "any stored energy has been released from the equipment"

#### `skills.logistics.s1_a` (novice, low, factual)

- Was: "I can lift and carry goods with a straight back and label a box, following the steps."
- Now: "I can lift and carry goods with the load close to my waist, a slight bend in my back, hips and knees, no twisting, and ask for help with anything heavy, and label a box, following the steps."
- Source: https://www.hse.gov.uk/msd/manual-handling/good-handling-technique.htm, last updated 2 April 2025; read 2026-11-14: "Slight bending of your back, hips and knees at the start of the lift is preferable to either fully flexing your back (stooping) or fully flexing your hips and knees (full/deep squatting)."

#### `skills.driving.s2_a` (beginner, high, legal)

- Was: "I can drive on a quiet road, keep a safe distance and obey the signs."
- Now: "I can drive on a quiet road with my learner's licence and a qualified driver supervising me, keep a safe distance and obey the signs."
- Source: https://www.gov.uk/driving-lessons-learning-to-drive, page metadata 2026-11-05; read 2026-11-14: "You must have a provisional driving licence for Great Britain or Northern Ireland when you’re learning to drive or ride. You must be supervised when you're learning to drive a car."

#### `skills.driving.s3_a` (competent, medium, legal)

- Was: "I can drive in traffic and on rough tracks without the guide, keeping a safe speed for the conditions."
- Now: "I can drive in traffic and on rough tracks without the guide, with a full licence for the vehicle, keeping a safe speed for the conditions."
- Note: Same content, reordered so the statement starts "I can".
- Source: https://www.gov.uk/driving-lessons-learning-to-drive, page metadata 2026-11-05; read 2026-11-14: "You must be supervised when you’re learning to drive a car."

#### `skills.flying.s3_a` (competent, medium, legal)

- Was: "I can plan a short flight, check the weather, and fly it and land without the guide."
- Now: "I can plan a short flight, check the weather, and fly it and land alone, once my instructor has signed me off for solo."
- Note: Same content, reordered so the statement starts "I can".
- Source: https://www.govinfo.gov/content/pkg/CFR-2025-title14-vol2/pdf/CFR-2025-title14-vol2-sec61-87.pdf, 14 CFR 61.87, official CFR 1-1-25 edition (govinfo); read 2026-11-14: "A student pilot may not operate an aircraft in solo flight unless that student pilot has received an endorsement in the student's logbook for the specific make and model aircraft to be flown by an authorized instructor"

#### `skills.flying.s4_a` (proficient, medium, order)

- Was: "I can deal with a failed engine or system and land safely, and change plans for weather."
- Added at beginner as `skills.flying.s2_c`: "I can handle a simulated engine failure and other emergencies with an instructor, and land safely."
- Now: "I can deal with a real engine or system failure in flight and land safely, and change plans for weather."
- Note: The simulated-failure half moves to Beginner as a new statement starting "I can"; Proficient keeps the real failure and the weather.
- Source: https://www.govinfo.gov/content/pkg/CFR-2025-title14-vol2/pdf/CFR-2025-title14-vol2-sec61-87.pdf, 14 CFR 61.87, official CFR 1-1-25 edition (govinfo); read 2026-11-14: "(11) Emergency procedures and equipment malfunctions; (12) Ground reference maneuvers; (13) Approaches to a landing area with simulated engine malfunctions; (14) Slips to a landing; and (15) Go-arounds."

#### `skills.flying.s6_a` (master, medium, legal)

- Was: "I can write a flying lesson and assess other pilots."
- Now: "I can hold a flight instructor certificate, write a flying lesson, and train and assess other pilots."
- Note: Same content, reordered so the statement starts "I can".
- Source: https://www.govinfo.gov/content/pkg/CFR-2025-title14-vol2/pdf/CFR-2025-title14-vol2-sec61-3.pdf, 14 CFR 61.3, official CFR 1-1-25 edition (govinfo); read 2026-11-14: "no person other than the holder of a flight instructor certificate issued under this part with the appropriate rating on that certificate may-- (i) Give training required to qualify a person for solo flight"

#### `skills.drone_operation.s1_a` (novice, low, legal)

- Was: "I can do the pre-flight check, keep the drone in sight, and fly a slow hover in an open area, following the steps."
- Now: "I can get the ID or registration the law needs for my drone before my first flight, then do the pre-flight check, keep the drone in sight, and fly a slow hover in an open area, following the steps."
- Note: Same content, reordered so the statement starts "I can".
- Source: https://www.caa.co.uk/drones/open-category/drone-code/getting-what-you-need-to-fly-legally/, UK CAA Drone and Model Aircraft Code, page build 2026-08-20; read 2026-11-14: "You must get a Flyer ID if you want to fly a drone or model aircraft that weighs 100g or above."

#### `skills.drone_operation.s3_b` (competent, medium, legal)

- Was: "I can fly safely around people and obstacles and keep clear of other aircraft."
- Now: "I can fly safely around obstacles, keep as far from people as the rules where I fly require, never fly over a crowd, and keep clear of other aircraft."
- Note: Generalised: the UK distance (50 m from uninvolved people for most drones, from the UK CAA source) stays in the source quote rather than the statement, which holds in any country.
- Source: https://www.caa.co.uk/drones/open-category/drone-code/where-you-can-fly-points-3-to-9/, UK CAA Drone and Model Aircraft Code, page build 2026-08-20; read 2026-11-14: "You must keep a minimum horizontal distance of 50m between your drone or model aircraft and people."

#### `skills.fire_performance.s3_b` (competent, high, order)

- Was: "I can run my full routine with unlit props first, and only then a lit one, always with a trained spotter and a fire blanket present."
- Added at competent as `skills.fire_performance.s3_c`: "I can fuel a prop safely, dipping the wick, spinning off the excess fuel into a spin-off bucket well away from the performance area, keeping the fuel capped and away from flame, and never using petrol."
- Now: "I can run my full routine with unlit props first, and only then a lit one, always with a trained spotter, a fire blanket or wet towel and an extinguisher present."
- Source: https://www.portland.gov/sites/default/files/2020-06/fir-3.07-fire-performance-art-080218-755010.pdf, Portland Fire & Rescue FMO Policy, Fire Performance Art, revision 8/18; read 2026-11-14: "Excess fuel shall be removed from devices in an approved manner before they are ignited or used in front of an audience. A "spin out bucket" shall be utilized to spin off any excess fuel"

#### `skills.fire_performance.s4_b` (proficient, medium, legal)

- Was: "I can adjust the setup for different spaces, such as indoors or crowded venues, and decide when not to perform."
- Now: "I can adjust the setup for different spaces, such as indoors or crowded venues, get the venue's and fire authority's permission (a permit in many places), and decide when not to perform."
- Source: https://www.portland.gov/sites/default/files/2020-06/fir-3.07-fire-performance-art-080218-755010.pdf, Portland Fire & Rescue FMO Policy, Fire Performance Art, revision 8/18; read 2026-11-14: "Each fire performance artist must obtain an "Individual Fire Performance Art Conditional Use Permit" for each performance location."

#### `skills.mediation.s3_b` (competent, medium, order)

- Was: "I can spot when a disagreement is too serious or unsafe for me and ask for help."
- Moved to beginner as `skills.mediation.s2_c`: "I can spot when a disagreement involves threats, violence or abuse, or is too serious for me, and get help instead of trying to settle it myself."
- Source: https://www.gov.uk/looking-after-children-divorce/mediation, read 2026-11-14 (no date captured): "You will not have to go to a MIAM in some circumstances, for example if there’s been domestic abuse."

## Sites that refused automated reading (not worked around)

- redcross.org and cpr.heart.org (403 on robots.txt); resus.org.uk (403 on the guidelines page);
  nssf.org (robots.txt disallows ClaudeBot); lnt.org (403); mass.gov and wlf.louisiana.gov (403);
  mdc.mo.gov (robots.txt disallows anthropic-ai).
- osha.gov refused group C (CloudFront 403, even on robots.txt), so group C cites the UK HSE for
  workplace rules and no OSHA number appears in its corrections; group A, earlier the same day, read
  OSHA pages and PDFs (robots.txt first).
- ecfr.gov redirected to a robot check: the aviation rules come from the official annual CFR edition
  on govinfo.gov (the Government Publishing Office's own edition), the geology rule from the NPS's
  own statement of it.
- faa.gov (403 on the student-pilot page); register-drones.caa.co.uk (robots.txt disallows all, the
  same Drone Code is on caa.co.uk); the National Bee Unit (BeeBase, 403: the Order on
  legislation.gov.uk is cited instead); the Queensland Electrical Safety Office (connection failed)
  and WorkSafe Queensland (robot check).
- For the medicine legal claim: California leginfo (403), Washington and Ohio code sites (robots.txt
  disallows every path), the Florida Senate (429 twice, not retried), NYSED (certificate failure).

## Gaps left for a later pass

Looked at, not refuted, and worth a statement or a source next time:

- **First aid**: no choking statement (back blows, abdominal thrusts). The defibrillator is now named
  in `skills.first_aid.s3_b` (send for one, follow its spoken instructions on a trainer), but there is
  no statement of using one on a person.
- **Fitness**: no pre-exercise health screening, and no warning signs beyond "something hurts" (chest
  pain, dizziness) in `skills.fitness.s1_a` or `s6_a`. The WHO activity targets for `s3_a` are only in
  the full guideline, not its fact sheet, so that statement was not refuted.
- **Medicine**: `skills.medicine.s4_a`'s legal reasoning needs a medical practice act quoted.
- **Grappling**: a choke should also be released if the partner goes limp (they may not be able to
  tap), `skills.grappling.s3_a`. Not sourced.
- **Climbing**: unroped scrambling at Competent (`s3_a`); trad leading and anchor building (`s5_a`)
  are usually learned with instruction.
- **Teaching and assessing at Master** (first aid, firearms, swimming, climbing, and others): formal
  courses need an instructor qualification. `skills.flying.s6_a` now says so; the rest do not.
- **Self defence**: body armour is restricted for some people and in some places (`s4_b`).
- **Food and water**: withdrawal times for food-animal medicines (`skills.animal_husbandry.s3_b` now
  says to keep to them, but only the prescription rule was quoted); US state bee-disease and
  registration laws; home brewing limits; `skills.mushroom_growing.s5_a` adds "only species I have
  positively identified" at Expert, which hints that earlier stages need not; a hunter education
  standard for humane shot placement (`skills.hunting.s3_c` rests on the ladder's own logic, the
  IHEA-USA standards returned 404).
- **Making and building**: textile mordant toxicity (`textiles.s3_b`), lye handling for soap
  (`chemistry.s3_a`), infrared eyewear at a glass furnace (`glassmaking.s1_a`), hearing protection
  for whip cracks, structural welder qualification (`welding.s5_a`), plumbing permits (`plumbing.s3_a`,
  `s5_a`), improvised parts on brakes, gas or pressure parts (`repair.s4_a`).
- **Travel**: licence classes for "any ground vehicle" (`driving.s5_a`), aircraft type endorsements
  (`flying.s4_b`), flying a drone beyond sight (`drone_operation.s5_a`).
- **Trading**: "stocking up before a shortage" (`trading.s4_a`) against price-gouging laws in
  emergencies, and against the mission.
- **Unquoted claims kept**: "solar panels are live in daylight" (`skills.electrical.s4_b`) is plain
  physics with no quoted source; the mediation finding's gov.uk source is indirect (mediation
  meetings are waived where there has been domestic abuse) and rests mainly on the stage model.
